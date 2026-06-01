use rickydata_git_relay::{
    FileRelayStore, GcsAuth, GcsRelayStore, HttpKfdbIndexSink, IndexedRelayStore, KfdbPrivateAuth,
    router,
};
use std::net::SocketAddr;
use std::path::PathBuf;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let store_dir = std::env::var_os("RICKYDATA_RELAY_STORE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(".rickydata-relay"));
    let addr = relay_addr_from_env()?.parse::<SocketAddr>()?;

    let listener = tokio::net::TcpListener::bind(addr).await?;
    let file_store = FileRelayStore::new(store_dir);
    if let Some(kfdb_url) = std::env::var_os("RICKYDATA_RELAY_KFDB_URL") {
        let token = std::env::var("RICKYDATA_RELAY_KFDB_BEARER_TOKEN").ok();
        let private_auth = relay_kfdb_private_auth()?;
        let index_sink = HttpKfdbIndexSink::new(kfdb_url.to_string_lossy(), token, private_auth)?;
        if let Some(bucket) = std::env::var_os("RICKYDATA_RELAY_GCS_BUCKET") {
            let store = gcs_store(bucket.to_string_lossy())?;
            axum::serve(listener, router(IndexedRelayStore::new(store, index_sink))).await?;
        } else {
            axum::serve(
                listener,
                router(IndexedRelayStore::new(file_store, index_sink)),
            )
            .await?;
        }
    } else if let Some(bucket) = std::env::var_os("RICKYDATA_RELAY_GCS_BUCKET") {
        axum::serve(listener, router(gcs_store(bucket.to_string_lossy())?)).await?;
    } else {
        axum::serve(listener, router(file_store)).await?;
    }
    Ok(())
}

fn relay_addr_from_env() -> anyhow::Result<String> {
    if let Ok(addr) = std::env::var("RICKYDATA_RELAY_ADDR") {
        return Ok(addr);
    }
    if let Ok(port) = std::env::var("PORT") {
        return Ok(format!("0.0.0.0:{port}"));
    }
    Ok("127.0.0.1:8080".to_string())
}

fn relay_kfdb_private_auth() -> anyhow::Result<KfdbPrivateAuth> {
    let derive_session_id = std::env::var("RICKYDATA_RELAY_KFDB_DERIVE_SESSION_ID")
        .or_else(|_| std::env::var("RICKYDATA_KFDB_DERIVE_SESSION_ID"))?;
    let derive_key = std::env::var("RICKYDATA_RELAY_KFDB_DERIVE_KEY")
        .or_else(|_| std::env::var("RICKYDATA_KFDB_DERIVE_KEY"))?;
    let wallet_address = std::env::var("RICKYDATA_RELAY_KFDB_WALLET_ADDRESS")
        .or_else(|_| std::env::var("RICKYDATA_KFDB_WALLET_ADDRESS"))
        .ok();
    Ok(KfdbPrivateAuth {
        derive_session_id,
        derive_key,
        wallet_address,
    })
}

fn gcs_store(bucket: impl AsRef<str>) -> anyhow::Result<GcsRelayStore> {
    let auth = if let Ok(token) = std::env::var("RICKYDATA_RELAY_GCS_BEARER_TOKEN") {
        GcsAuth::Bearer(token)
    } else if let Ok(token_url) = std::env::var("RICKYDATA_RELAY_GCS_METADATA_TOKEN_URL") {
        GcsAuth::Metadata { token_url }
    } else {
        GcsAuth::Metadata {
            token_url: "http://metadata.google.internal/computeMetadata/v1/instance/service-accounts/default/token"
                .to_string(),
        }
    };
    if let Ok(api_base_url) = std::env::var("RICKYDATA_RELAY_GCS_API_BASE_URL") {
        let upload_base_url =
            std::env::var("RICKYDATA_RELAY_GCS_UPLOAD_BASE_URL").unwrap_or(api_base_url.clone());
        Ok(GcsRelayStore::with_base_urls(
            bucket.as_ref(),
            api_base_url,
            upload_base_url,
            auth,
        )?)
    } else {
        Ok(GcsRelayStore::new(bucket.as_ref(), auth)?)
    }
}
