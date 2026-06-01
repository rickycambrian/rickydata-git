use axum::extract::{Path as AxumPath, Request, State};
use axum::http::StatusCode;
use axum::http::header::AUTHORIZATION;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use percent_encoding::{NON_ALPHANUMERIC, utf8_percent_encode};
use rickydata_git_core::{
    CanonicalObject, canonical_json, canonical_object_id, stable_json_hash, verify_signature,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};
use uuid::Uuid;

#[derive(Debug, thiserror::Error)]
pub enum RelayError {
    #[error("invalid object `{object_id}`: {message}")]
    InvalidObject { object_id: String, message: String },
    #[error("object `{object_id}` already exists with different bytes")]
    ObjectConflict { object_id: String },
    #[error("failed to read or write relay store: {0}")]
    Io(#[from] std::io::Error),
    #[error("failed to parse relay JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("failed to hash relay JSON: {0}")]
    Core(#[from] rickydata_git_core::CoreError),
    #[error("failed to index relay bundle in KFDB: {0}")]
    KfdbIndex(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct HealthReport {
    pub status: String,
    pub service: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RelayErrorReport {
    pub status: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct BundlePushRequest {
    pub repo_id: String,
    pub idempotency_key: String,
    pub objects: Vec<CanonicalObject<Value>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct BundlePushReport {
    pub status: String,
    pub repo_id: String,
    pub idempotency_key: String,
    pub accepted_object_count: usize,
    pub duplicate_object_count: usize,
    pub object_ids: Vec<String>,
    pub bundle_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct BundlePullRequest {
    pub repo_id: String,
    #[serde(default)]
    pub known_object_ids: Vec<String>,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct BundlePullReport {
    pub status: String,
    pub repo_id: String,
    pub object_count: usize,
    pub remaining_object_count: usize,
    pub objects: Vec<CanonicalObject<Value>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct BundleValidationReport {
    pub status: String,
    pub repo_id: String,
    pub object_count: usize,
    pub object_ids: Vec<String>,
    pub bundle_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RepoRelayStatusReport {
    pub status: String,
    pub repo_id: String,
    pub object_count: usize,
    pub object_ids_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct KfdbWriteBatch {
    pub operations: Vec<Value>,
    pub skip_embedding: bool,
}

pub trait KfdbIndexSink: Send + Sync {
    fn write_batch(&self, batch: &KfdbWriteBatch) -> Result<(), RelayError>;
}

const KFDB_INDEX_BATCH_OPERATION_LIMIT: usize = 100;

#[derive(Debug, Clone)]
pub struct HttpKfdbIndexSink {
    write_url: String,
    bearer_token: Option<String>,
    private_auth: KfdbPrivateAuth,
    client: reqwest::blocking::Client,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KfdbPrivateAuth {
    pub derive_session_id: String,
    pub derive_key: String,
    pub wallet_address: Option<String>,
}

#[derive(Debug, Clone)]
pub struct IndexedRelayStore<S, I> {
    inner: S,
    index_sink: I,
}

pub trait RelayStore: Send + Sync {
    fn validate_bundle(
        &self,
        request: &BundlePushRequest,
    ) -> Result<BundleValidationReport, RelayError>;
    fn push_bundle(&self, request: &BundlePushRequest) -> Result<BundlePushReport, RelayError>;
    fn pull_bundle(&self, request: &BundlePullRequest) -> Result<BundlePullReport, RelayError>;
    fn read_object(
        &self,
        repo_id: &str,
        object_id: &str,
    ) -> Result<CanonicalObject<Value>, RelayError>;
    fn list_object_ids(&self, repo_id: &str) -> Result<Vec<String>, RelayError>;

    fn repo_status(&self, repo_id: &str) -> Result<RepoRelayStatusReport, RelayError> {
        let object_ids = self.list_object_ids(repo_id)?;
        Ok(RepoRelayStatusReport {
            status: "ok".to_string(),
            repo_id: repo_id.to_string(),
            object_count: object_ids.len(),
            object_ids_hash: bundle_hash(&object_ids)?,
        })
    }
}

#[derive(Debug, Clone)]
pub struct FileRelayStore {
    root: PathBuf,
}

#[derive(Debug, Clone)]
pub enum GcsAuth {
    None,
    Bearer(String),
    Metadata { token_url: String },
}

#[derive(Debug, Clone)]
pub struct GcsRelayStore {
    bucket: String,
    api_base_url: String,
    upload_base_url: String,
    auth: GcsAuth,
    client: reqwest::blocking::Client,
    token_cache: Arc<Mutex<Option<CachedGcsToken>>>,
}

#[derive(Debug, Clone)]
struct CachedGcsToken {
    token: String,
    expires_at: Instant,
}

#[derive(Clone)]
struct RelayState {
    store: Arc<dyn RelayStore>,
}

pub fn router(store: impl RelayStore + 'static) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/v1/repos/{repo_id}/status", get(repo_status))
        .route(
            "/v1/repos/{repo_id}/bundles/validate",
            post(validate_bundle),
        )
        .route("/v1/repos/{repo_id}/bundles/push", post(push_bundle))
        .route("/v1/repos/{repo_id}/bundles/pull", post(pull_bundle))
        .route("/v1/repos/{repo_id}/objects/{object_id}", get(get_object))
        .with_state(RelayState {
            store: Arc::new(store),
        })
}

/// Build the relay router with an optional bearer-token gate.
///
/// The relay is the *secondary* cross-fleet channel (the primary is a shared
/// private git repo over `sync push/pull`). When `auth_token` is `Some`, every
/// route except `/health` requires `Authorization: Bearer <token>`; mismatch or
/// absence returns 401. When `None`, the relay is open (back-compat for
/// local/dev use) — callers should warn loudly in that case.
pub fn router_with_auth(store: impl RelayStore + 'static, auth_token: Option<String>) -> Router {
    router(store).layer(axum::middleware::from_fn_with_state(
        AuthConfig { token: auth_token },
        require_bearer_auth,
    ))
}

#[derive(Clone)]
struct AuthConfig {
    token: Option<String>,
}

async fn require_bearer_auth(
    State(config): State<AuthConfig>,
    request: Request,
    next: Next,
) -> Result<Response, AuthError> {
    // Auth disabled when no token is configured (open relay).
    let Some(expected) = config.token.as_ref() else {
        return Ok(next.run(request).await);
    };
    // Liveness probes must always reach `/health` unauthenticated.
    if request.uri().path() == "/health" {
        return Ok(next.run(request).await);
    }
    let provided = request
        .headers()
        .get(AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "));
    match provided {
        Some(token) if token == expected => Ok(next.run(request).await),
        _ => Err(AuthError),
    }
}

#[derive(Debug)]
pub struct AuthError;

impl IntoResponse for AuthError {
    fn into_response(self) -> Response {
        (
            StatusCode::UNAUTHORIZED,
            Json(RelayErrorReport {
                status: "error".to_string(),
                message: "missing or invalid bearer token".to_string(),
            }),
        )
            .into_response()
    }
}

async fn health() -> Json<HealthReport> {
    Json(HealthReport {
        status: "ok".to_string(),
        service: "rickydata-git-relay".to_string(),
    })
}

async fn repo_status(
    State(state): State<RelayState>,
    AxumPath(repo_id): AxumPath<String>,
) -> Result<Json<RepoRelayStatusReport>, RelayHttpError> {
    Ok(Json(state.store.repo_status(&repo_id)?))
}

async fn validate_bundle(
    State(state): State<RelayState>,
    AxumPath(repo_id): AxumPath<String>,
    Json(mut request): Json<BundlePushRequest>,
) -> Result<Json<BundleValidationReport>, RelayHttpError> {
    ensure_repo_id(&repo_id, &mut request.repo_id)?;
    Ok(Json(state.store.validate_bundle(&request)?))
}

async fn push_bundle(
    State(state): State<RelayState>,
    AxumPath(repo_id): AxumPath<String>,
    Json(mut request): Json<BundlePushRequest>,
) -> Result<Json<BundlePushReport>, RelayHttpError> {
    ensure_repo_id(&repo_id, &mut request.repo_id)?;
    Ok(Json(state.store.push_bundle(&request)?))
}

async fn pull_bundle(
    State(state): State<RelayState>,
    AxumPath(repo_id): AxumPath<String>,
    Json(mut request): Json<BundlePullRequest>,
) -> Result<Json<BundlePullReport>, RelayHttpError> {
    ensure_repo_id(&repo_id, &mut request.repo_id)?;
    Ok(Json(state.store.pull_bundle(&request)?))
}

async fn get_object(
    State(state): State<RelayState>,
    AxumPath((repo_id, object_id)): AxumPath<(String, String)>,
) -> Result<Json<CanonicalObject<Value>>, RelayHttpError> {
    Ok(Json(state.store.read_object(&repo_id, &object_id)?))
}

fn ensure_repo_id(path_repo_id: &str, body_repo_id: &mut String) -> Result<(), RelayHttpError> {
    if body_repo_id.is_empty() {
        *body_repo_id = path_repo_id.to_string();
        return Ok(());
    }
    if body_repo_id != path_repo_id {
        return Err(RelayHttpError {
            status: StatusCode::BAD_REQUEST,
            message: format!(
                "body repo_id `{}` does not match path repo_id `{path_repo_id}`",
                body_repo_id
            ),
        });
    }
    Ok(())
}

#[derive(Debug)]
pub struct RelayHttpError {
    status: StatusCode,
    message: String,
}

impl From<RelayError> for RelayHttpError {
    fn from(error: RelayError) -> Self {
        let status = match error {
            RelayError::InvalidObject { .. } => StatusCode::BAD_REQUEST,
            RelayError::ObjectConflict { .. } => StatusCode::CONFLICT,
            RelayError::Io(_)
            | RelayError::Json(_)
            | RelayError::Core(_)
            | RelayError::KfdbIndex(_) => StatusCode::INTERNAL_SERVER_ERROR,
        };
        Self {
            status,
            message: error.to_string(),
        }
    }
}

impl IntoResponse for RelayHttpError {
    fn into_response(self) -> Response {
        (
            self.status,
            Json(RelayErrorReport {
                status: "error".to_string(),
                message: self.message,
            }),
        )
            .into_response()
    }
}

impl FileRelayStore {
    pub fn new(root: impl AsRef<Path>) -> Self {
        Self {
            root: root.as_ref().to_path_buf(),
        }
    }
}

impl HttpKfdbIndexSink {
    pub fn new(
        base_url: impl AsRef<str>,
        bearer_token: Option<String>,
        private_auth: KfdbPrivateAuth,
    ) -> Result<Self, RelayError> {
        let write_url = format!("{}/api/v1/write", base_url.as_ref().trim_end_matches('/'));
        let client = reqwest::blocking::Client::builder()
            .http1_only()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|error| RelayError::KfdbIndex(error.to_string()))?;
        Ok(Self {
            write_url,
            bearer_token,
            private_auth,
            client,
        })
    }
}

impl KfdbIndexSink for HttpKfdbIndexSink {
    fn write_batch(&self, batch: &KfdbWriteBatch) -> Result<(), RelayError> {
        let mut request = self.client.post(&self.write_url).json(batch);
        if let Some(token) = self.bearer_token.as_ref() {
            request = request.bearer_auth(token);
        }
        request = request
            .header("x-derive-session-id", &self.private_auth.derive_session_id)
            .header("x-derive-key", &self.private_auth.derive_key);
        if let Some(wallet_address) = self.private_auth.wallet_address.as_ref() {
            request = request.header("x-wallet-address", wallet_address);
        }
        let response = request
            .send()
            .map_err(|error| RelayError::KfdbIndex(error.to_string()))?;
        let status = response.status();
        if !status.is_success() {
            let body = response.text().unwrap_or_default();
            return Err(RelayError::KfdbIndex(format!(
                "KFDB write returned {status}: {body}"
            )));
        }
        Ok(())
    }
}

impl<S, I> IndexedRelayStore<S, I> {
    pub fn new(inner: S, index_sink: I) -> Self {
        Self { inner, index_sink }
    }
}

impl<S, I> RelayStore for IndexedRelayStore<S, I>
where
    S: RelayStore,
    I: KfdbIndexSink,
{
    fn validate_bundle(
        &self,
        request: &BundlePushRequest,
    ) -> Result<BundleValidationReport, RelayError> {
        self.inner.validate_bundle(request)
    }

    fn push_bundle(&self, request: &BundlePushRequest) -> Result<BundlePushReport, RelayError> {
        let report = self.inner.push_bundle(request)?;
        let batch = kfdb_index_batch(&request.repo_id, &report.bundle_hash, &request.objects)?;
        for operations in batch.operations.chunks(KFDB_INDEX_BATCH_OPERATION_LIMIT) {
            let chunk = KfdbWriteBatch {
                operations: operations.to_vec(),
                skip_embedding: batch.skip_embedding,
            };
            if let Err(error) = self.index_sink.write_batch(&chunk) {
                eprintln!(
                    "rickydata-git-relay: KFDB projection failed after canonical bundle persistence: {error}"
                );
                break;
            }
        }
        Ok(report)
    }

    fn pull_bundle(&self, request: &BundlePullRequest) -> Result<BundlePullReport, RelayError> {
        self.inner.pull_bundle(request)
    }

    fn read_object(
        &self,
        repo_id: &str,
        object_id: &str,
    ) -> Result<CanonicalObject<Value>, RelayError> {
        self.inner.read_object(repo_id, object_id)
    }

    fn list_object_ids(&self, repo_id: &str) -> Result<Vec<String>, RelayError> {
        self.inner.list_object_ids(repo_id)
    }

    fn repo_status(&self, repo_id: &str) -> Result<RepoRelayStatusReport, RelayError> {
        self.inner.repo_status(repo_id)
    }
}

enum GcsPutResult {
    Written,
    AlreadyExists,
}

impl GcsRelayStore {
    pub fn new(bucket: impl Into<String>, auth: GcsAuth) -> Result<Self, RelayError> {
        Self::with_base_urls(
            bucket,
            "https://storage.googleapis.com",
            "https://storage.googleapis.com",
            auth,
        )
    }

    pub fn with_base_urls(
        bucket: impl Into<String>,
        api_base_url: impl Into<String>,
        upload_base_url: impl Into<String>,
        auth: GcsAuth,
    ) -> Result<Self, RelayError> {
        let client = reqwest::blocking::Client::builder()
            .http1_only()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|error| RelayError::Io(io::Error::other(error)))?;
        Ok(Self {
            bucket: bucket.into(),
            api_base_url: api_base_url.into().trim_end_matches('/').to_string(),
            upload_base_url: upload_base_url.into().trim_end_matches('/').to_string(),
            auth,
            client,
            token_cache: Arc::new(Mutex::new(None)),
        })
    }

    fn object_name(repo_id: &str, object_id: &str) -> String {
        format!("repos/{repo_id}/objects/{}.json", key_hash(object_id))
    }

    fn idempotency_name(repo_id: &str, idempotency_key: &str) -> String {
        format!(
            "repos/{repo_id}/idempotency/{}.json",
            key_hash(idempotency_key)
        )
    }

    fn object_url(&self, name: &str) -> String {
        let encoded_name = utf8_percent_encode(name, NON_ALPHANUMERIC).to_string();
        format!(
            "{}/storage/v1/b/{}/o/{}",
            self.api_base_url, self.bucket, encoded_name
        )
    }

    fn upload_url(&self, name: &str) -> Result<reqwest::Url, RelayError> {
        let mut url = reqwest::Url::parse(&format!(
            "{}/upload/storage/v1/b/{}/o",
            self.upload_base_url, self.bucket
        ))
        .map_err(|error| RelayError::Io(io::Error::new(io::ErrorKind::InvalidInput, error)))?;
        url.query_pairs_mut()
            .append_pair("uploadType", "media")
            .append_pair("ifGenerationMatch", "0")
            .append_pair("name", name);
        Ok(url)
    }

    fn list_url(&self, prefix: &str, page_token: Option<&str>) -> Result<reqwest::Url, RelayError> {
        let mut url = reqwest::Url::parse(&format!(
            "{}/storage/v1/b/{}/o",
            self.api_base_url, self.bucket
        ))
        .map_err(|error| RelayError::Io(io::Error::new(io::ErrorKind::InvalidInput, error)))?;
        url.query_pairs_mut().append_pair("prefix", prefix);
        if let Some(page_token) = page_token {
            url.query_pairs_mut().append_pair("pageToken", page_token);
        }
        Ok(url)
    }

    fn auth_token(&self) -> Result<Option<String>, RelayError> {
        match &self.auth {
            GcsAuth::None => Ok(None),
            GcsAuth::Bearer(token) => Ok(Some(token.clone())),
            GcsAuth::Metadata { token_url } => {
                if let Some(cached) = self.cached_token()? {
                    return Ok(Some(cached));
                }
                let response = self.fetch_metadata_token(token_url)?;
                let token = response
                    .get("access_token")
                    .and_then(Value::as_str)
                    .ok_or_else(|| {
                        RelayError::Io(io::Error::new(
                            io::ErrorKind::InvalidData,
                            "GCS metadata token response missing access_token",
                        ))
                    })?;
                let expires_in = response
                    .get("expires_in")
                    .and_then(Value::as_u64)
                    .unwrap_or(300);
                let cached = CachedGcsToken {
                    token: token.to_string(),
                    expires_at: Instant::now()
                        + Duration::from_secs(expires_in.saturating_sub(60).max(1)),
                };
                *self
                    .token_cache
                    .lock()
                    .map_err(|_| RelayError::Io(io::Error::other("GCS token cache poisoned")))? =
                    Some(cached.clone());
                Ok(Some(cached.token))
            }
        }
    }

    fn cached_token(&self) -> Result<Option<String>, RelayError> {
        let cache = self
            .token_cache
            .lock()
            .map_err(|_| RelayError::Io(io::Error::other("GCS token cache poisoned")))?;
        Ok(cache
            .as_ref()
            .filter(|cached| cached.expires_at > Instant::now() + Duration::from_secs(30))
            .map(|cached| cached.token.clone()))
    }

    fn fetch_metadata_token(&self, token_url: &str) -> Result<Value, RelayError> {
        let mut errors = Vec::new();
        for candidate in metadata_token_url_candidates(token_url) {
            let response = self.send_with_retry(
                self.client
                    .get(&candidate)
                    .header("Metadata-Flavor", "Google"),
            );
            match response {
                Ok(response) => {
                    return response
                        .error_for_status()
                        .map_err(|error| RelayError::Io(io::Error::other(error)))?
                        .json()
                        .map_err(|error| RelayError::Io(io::Error::other(error)));
                }
                Err(error) => errors.push(error.to_string()),
            }
        }
        Err(RelayError::Io(io::Error::other(format!(
            "failed to fetch GCS metadata token: {}",
            errors.join("; ")
        ))))
    }

    fn with_auth(
        &self,
        request: reqwest::blocking::RequestBuilder,
    ) -> Result<reqwest::blocking::RequestBuilder, RelayError> {
        Ok(match self.auth_token()? {
            Some(token) => request.bearer_auth(token),
            None => request,
        })
    }

    fn get_bytes(&self, name: &str) -> Result<Vec<u8>, RelayError> {
        let response = self
            .with_auth(
                self.client
                    .get(self.object_url(name))
                    .query(&[("alt", "media")]),
            )?
            .send_with_gcs_retry(self)?;
        if response.status() == StatusCode::NOT_FOUND {
            return Err(RelayError::Io(io::Error::new(
                io::ErrorKind::NotFound,
                format!("GCS object `{name}` was not found"),
            )));
        }
        Ok(response
            .error_for_status()
            .map_err(|error| RelayError::Io(io::Error::other(error)))?
            .bytes()
            .map_err(|error| RelayError::Io(io::Error::other(error)))?
            .to_vec())
    }

    fn put_bytes_if_absent(&self, name: &str, bytes: &[u8]) -> Result<GcsPutResult, RelayError> {
        let response = self
            .with_auth(
                self.client
                    .post(self.upload_url(name)?)
                    .body(bytes.to_vec()),
            )?
            .send_with_gcs_retry(self)?;
        if response.status().is_success() {
            return Ok(GcsPutResult::Written);
        }
        if response.status() == StatusCode::PRECONDITION_FAILED
            || response.status() == StatusCode::CONFLICT
        {
            return Ok(GcsPutResult::AlreadyExists);
        }
        Err(RelayError::Io(io::Error::other(format!(
            "GCS write for `{name}` returned {}",
            response.status()
        ))))
    }

    fn read_json_object<T: for<'de> Deserialize<'de>>(&self, name: &str) -> Result<T, RelayError> {
        Ok(serde_json::from_slice(&self.get_bytes(name)?)?)
    }

    fn put_json_if_absent<T: Serialize>(
        &self,
        name: &str,
        value: &T,
    ) -> Result<GcsPutResult, RelayError> {
        self.put_bytes_if_absent(name, &serde_json::to_vec_pretty(value)?)
    }

    fn list_names(&self, prefix: &str) -> Result<Vec<String>, RelayError> {
        let mut names = Vec::new();
        let mut page_token = None;
        loop {
            let response: Value = self
                .with_auth(
                    self.client
                        .get(self.list_url(prefix, page_token.as_deref())?),
                )?
                .send_with_gcs_retry(self)?
                .error_for_status()
                .map_err(|error| RelayError::Io(io::Error::other(error)))?
                .json()
                .map_err(|error| RelayError::Io(io::Error::other(error)))?;
            if let Some(items) = response.get("items").and_then(Value::as_array) {
                for item in items {
                    if let Some(name) = item.get("name").and_then(Value::as_str) {
                        names.push(name.to_string());
                    }
                }
            }
            page_token = response
                .get("nextPageToken")
                .and_then(Value::as_str)
                .map(ToOwned::to_owned);
            if page_token.is_none() {
                break;
            }
        }
        Ok(names)
    }

    fn send_with_retry(
        &self,
        request: reqwest::blocking::RequestBuilder,
    ) -> Result<reqwest::blocking::Response, RelayError> {
        let mut last_error = None;
        for attempt in 0..3 {
            let Some(builder) = request.try_clone() else {
                return Err(RelayError::Io(io::Error::other(
                    "GCS request could not be retried because it is not cloneable",
                )));
            };
            match builder.send() {
                Ok(response) if response.status().is_server_error() && attempt < 2 => {
                    last_error = Some(format!("GCS returned {}", response.status()));
                    thread::sleep(Duration::from_millis(150 * (attempt + 1) as u64));
                }
                Ok(response) => return Ok(response),
                Err(error) if attempt < 2 => {
                    last_error = Some(error.to_string());
                    thread::sleep(Duration::from_millis(150 * (attempt + 1) as u64));
                }
                Err(error) => return Err(RelayError::Io(io::Error::other(error))),
            }
        }
        Err(RelayError::Io(io::Error::other(last_error.unwrap_or_else(
            || "GCS request failed after retries".to_string(),
        ))))
    }
}

trait GcsRequestRetry {
    fn send_with_gcs_retry(
        self,
        store: &GcsRelayStore,
    ) -> Result<reqwest::blocking::Response, RelayError>;
}

impl GcsRequestRetry for reqwest::blocking::RequestBuilder {
    fn send_with_gcs_retry(
        self,
        store: &GcsRelayStore,
    ) -> Result<reqwest::blocking::Response, RelayError> {
        store.send_with_retry(self)
    }
}

fn metadata_token_url_candidates(token_url: &str) -> Vec<String> {
    let mut urls = vec![token_url.to_string()];
    if token_url.contains("metadata.google.internal") {
        urls.push(token_url.replace("metadata.google.internal", "169.254.169.254"));
    }
    urls
}

impl RelayStore for GcsRelayStore {
    fn repo_status(&self, repo_id: &str) -> Result<RepoRelayStatusReport, RelayError> {
        let prefix = format!("repos/{repo_id}/objects/");
        let mut names = self.list_names(&prefix)?;
        names.sort();
        Ok(RepoRelayStatusReport {
            status: "ok".to_string(),
            repo_id: repo_id.to_string(),
            object_count: names.len(),
            object_ids_hash: bundle_hash(&names)?,
        })
    }

    fn validate_bundle(
        &self,
        request: &BundlePushRequest,
    ) -> Result<BundleValidationReport, RelayError> {
        bundle_validation_report(request)
    }

    fn push_bundle(&self, request: &BundlePushRequest) -> Result<BundlePushReport, RelayError> {
        bundle_validation_report(request)?;
        let idempotency_name = Self::idempotency_name(&request.repo_id, &request.idempotency_key);
        if let Ok(report) = self.read_json_object::<BundlePushReport>(&idempotency_name) {
            return Ok(report);
        }

        let mut accepted_object_count = 0;
        let mut duplicate_object_count = 0;
        let existing_names = self
            .list_names(&format!("repos/{}/objects/", request.repo_id))?
            .into_iter()
            .collect::<BTreeSet<_>>();
        let object_ids = request
            .objects
            .iter()
            .map(|object| object.object_id.clone())
            .collect::<Vec<_>>();
        for object in &request.objects {
            let name = Self::object_name(&request.repo_id, &object.object_id);
            if existing_names.contains(&name) {
                duplicate_object_count += 1;
                continue;
            }
            let bytes = canonical_object_bytes(object)?;
            match self.put_bytes_if_absent(&name, &bytes)? {
                GcsPutResult::Written => accepted_object_count += 1,
                GcsPutResult::AlreadyExists => duplicate_object_count += 1,
            }
        }

        let report = BundlePushReport {
            status: "ok".to_string(),
            repo_id: request.repo_id.clone(),
            idempotency_key: request.idempotency_key.clone(),
            accepted_object_count,
            duplicate_object_count,
            object_ids: object_ids.clone(),
            bundle_hash: bundle_hash(&object_ids)?,
        };
        match self.put_json_if_absent(&idempotency_name, &report)? {
            GcsPutResult::Written => Ok(report),
            GcsPutResult::AlreadyExists => self.read_json_object(&idempotency_name),
        }
    }

    fn pull_bundle(&self, request: &BundlePullRequest) -> Result<BundlePullReport, RelayError> {
        let known_names = request
            .known_object_ids
            .iter()
            .map(|object_id| Self::object_name(&request.repo_id, object_id))
            .collect::<BTreeSet<_>>();
        let prefix = format!("repos/{}/objects/", request.repo_id);
        let mut missing_names = self
            .list_names(&prefix)?
            .into_iter()
            .filter(|name| !known_names.contains(name))
            .collect::<Vec<_>>();
        missing_names.sort();
        let total_missing = missing_names.len();
        let limit = request.limit.unwrap_or(total_missing);
        let remaining_object_count = total_missing.saturating_sub(limit);
        let mut objects = Vec::new();
        for name in missing_names.into_iter().take(limit) {
            let object: CanonicalObject<Value> = serde_json::from_slice(&self.get_bytes(&name)?)?;
            validate_stored_object(&request.repo_id, &object)?;
            objects.push(object);
        }
        Ok(BundlePullReport {
            status: "ok".to_string(),
            repo_id: request.repo_id.clone(),
            object_count: objects.len(),
            remaining_object_count,
            objects,
        })
    }

    fn read_object(
        &self,
        repo_id: &str,
        object_id: &str,
    ) -> Result<CanonicalObject<Value>, RelayError> {
        let object =
            serde_json::from_slice(&self.get_bytes(&Self::object_name(repo_id, object_id))?)?;
        validate_stored_object(repo_id, &object)?;
        Ok(object)
    }

    fn list_object_ids(&self, repo_id: &str) -> Result<Vec<String>, RelayError> {
        let prefix = format!("repos/{repo_id}/objects/");
        let mut object_ids = Vec::new();
        for name in self.list_names(&prefix)? {
            let object: CanonicalObject<Value> = serde_json::from_slice(&self.get_bytes(&name)?)?;
            validate_stored_object(repo_id, &object)?;
            object_ids.push(object.object_id);
        }
        object_ids.sort();
        Ok(object_ids)
    }
}

impl RelayStore for FileRelayStore {
    fn validate_bundle(
        &self,
        request: &BundlePushRequest,
    ) -> Result<BundleValidationReport, RelayError> {
        let object_ids = validated_object_ids(&request.repo_id, &request.objects)?;
        let bundle_hash = bundle_hash(&object_ids)?;
        Ok(BundleValidationReport {
            status: "ok".to_string(),
            repo_id: request.repo_id.clone(),
            object_count: request.objects.len(),
            object_ids,
            bundle_hash,
        })
    }

    fn push_bundle(&self, request: &BundlePushRequest) -> Result<BundlePushReport, RelayError> {
        let validation = self.validate_bundle(request)?;
        let repo_dir = self.repo_dir(&request.repo_id);
        let idempotency_path = repo_dir
            .join("idempotency")
            .join(format!("{}.json", key_hash(&request.idempotency_key)));
        if idempotency_path.exists() {
            return read_json(&idempotency_path);
        }

        let object_bytes = request
            .objects
            .iter()
            .map(canonical_object_bytes)
            .collect::<Result<Vec<_>, _>>()?;
        for (object, bytes) in request.objects.iter().zip(object_bytes.iter()) {
            let path = self.object_path(&request.repo_id, &object.object_id)?;
            if path.exists() && std::fs::read(&path)? != *bytes {
                return Err(RelayError::ObjectConflict {
                    object_id: object.object_id.clone(),
                });
            }
        }

        let mut accepted_object_count = 0;
        let mut duplicate_object_count = 0;
        for (object, bytes) in request.objects.iter().zip(object_bytes.iter()) {
            let path = self.object_path(&request.repo_id, &object.object_id)?;
            if path.exists() {
                duplicate_object_count += 1;
                continue;
            }
            write_bytes(&path, bytes)?;
            accepted_object_count += 1;
        }

        let report = BundlePushReport {
            status: "ok".to_string(),
            repo_id: request.repo_id.clone(),
            idempotency_key: request.idempotency_key.clone(),
            accepted_object_count,
            duplicate_object_count,
            object_ids: validation.object_ids,
            bundle_hash: validation.bundle_hash,
        };
        write_json(&idempotency_path, &report)?;
        Ok(report)
    }

    fn pull_bundle(&self, request: &BundlePullRequest) -> Result<BundlePullReport, RelayError> {
        let known = request
            .known_object_ids
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let mut object_ids = self.list_object_ids(&request.repo_id)?;
        object_ids.retain(|object_id| !known.contains(object_id));

        let limit = request.limit.unwrap_or(object_ids.len());
        let remaining_object_count = object_ids.len().saturating_sub(limit);
        let selected = object_ids.into_iter().take(limit).collect::<Vec<_>>();
        let objects = selected
            .iter()
            .map(|object_id| self.read_object(&request.repo_id, object_id))
            .collect::<Result<Vec<_>, _>>()?;

        Ok(BundlePullReport {
            status: "ok".to_string(),
            repo_id: request.repo_id.clone(),
            object_count: objects.len(),
            remaining_object_count,
            objects,
        })
    }

    fn read_object(
        &self,
        repo_id: &str,
        object_id: &str,
    ) -> Result<CanonicalObject<Value>, RelayError> {
        read_json(&self.object_path(repo_id, object_id)?)
    }

    fn list_object_ids(&self, repo_id: &str) -> Result<Vec<String>, RelayError> {
        let objects_dir = self.repo_dir(repo_id).join("objects").join("sha256");
        if !objects_dir.exists() {
            return Ok(Vec::new());
        }

        let mut object_ids = Vec::new();
        for prefix in std::fs::read_dir(objects_dir)? {
            let prefix = prefix?;
            if !prefix.file_type()?.is_dir() {
                continue;
            }
            for entry in std::fs::read_dir(prefix.path())? {
                let entry = entry?;
                if !entry.file_type()?.is_file() {
                    continue;
                }
                let Some(name) = entry.file_name().to_str().map(str::to_string) else {
                    continue;
                };
                let Some(hex) = name.strip_suffix(".json") else {
                    continue;
                };
                object_ids.push(format!("sha256:{hex}"));
            }
        }
        object_ids.sort();
        Ok(object_ids)
    }
}

impl FileRelayStore {
    fn repo_dir(&self, repo_id: &str) -> PathBuf {
        self.root.join("repos").join(key_hash(repo_id))
    }

    fn object_path(&self, repo_id: &str, object_id: &str) -> Result<PathBuf, RelayError> {
        let Some(hex) = object_id.strip_prefix("sha256:") else {
            return Err(RelayError::InvalidObject {
                object_id: object_id.to_string(),
                message: "expected sha256:<64 lowercase hex chars>".to_string(),
            });
        };
        if hex.len() != 64
            || !hex
                .chars()
                .all(|ch| ch.is_ascii_hexdigit() && !ch.is_ascii_uppercase())
        {
            return Err(RelayError::InvalidObject {
                object_id: object_id.to_string(),
                message: "expected sha256:<64 lowercase hex chars>".to_string(),
            });
        }
        Ok(self
            .repo_dir(repo_id)
            .join("objects")
            .join("sha256")
            .join(&hex[0..2])
            .join(format!("{hex}.json")))
    }
}

fn validated_object_ids(
    repo_id: &str,
    objects: &[CanonicalObject<Value>],
) -> Result<Vec<String>, RelayError> {
    validated_object_ids_with_policy(
        repo_id,
        objects,
        signature_enforcement_enabled(),
        &legacy_unsigned_repo_ids(),
    )
}

fn validated_object_ids_with_policy(
    repo_id: &str,
    objects: &[CanonicalObject<Value>],
    enforce_signatures: bool,
    legacy_unsigned_repos: &BTreeSet<String>,
) -> Result<Vec<String>, RelayError> {
    let mut seen = BTreeSet::new();
    let mut object_ids = Vec::new();
    for object in objects {
        validate_stored_object_with_policy(
            repo_id,
            object,
            enforce_signatures,
            legacy_unsigned_repos,
        )?;
        if !seen.insert(object.object_id.clone()) {
            continue;
        }
        object_ids.push(object.object_id.clone());
    }
    Ok(object_ids)
}

fn bundle_validation_report(
    request: &BundlePushRequest,
) -> Result<BundleValidationReport, RelayError> {
    let object_ids = validated_object_ids(&request.repo_id, &request.objects)?;
    let bundle_hash = bundle_hash(&object_ids)?;
    Ok(BundleValidationReport {
        status: "ok".to_string(),
        repo_id: request.repo_id.clone(),
        object_count: request.objects.len(),
        object_ids,
        bundle_hash,
    })
}

pub const ENFORCE_SIGNATURES_ENV: &str = "RICKYDATA_RELAY_ENFORCE_SIGNATURES";
pub const LEGACY_UNSIGNED_REPOS_ENV: &str = "RICKYDATA_RELAY_LEGACY_UNSIGNED_REPOS";

fn signature_enforcement_enabled() -> bool {
    matches!(
        std::env::var(ENFORCE_SIGNATURES_ENV)
            .ok()
            .as_deref()
            .map(str::trim),
        Some("1") | Some("true") | Some("TRUE") | Some("yes") | Some("on")
    )
}

fn legacy_unsigned_repo_ids() -> BTreeSet<String> {
    std::env::var(LEGACY_UNSIGNED_REPOS_ENV)
        .ok()
        .into_iter()
        .flat_map(|value| {
            value
                .split(',')
                .map(str::trim)
                .filter(|repo_id| !repo_id.is_empty())
                .map(ToOwned::to_owned)
                .collect::<Vec<_>>()
        })
        .collect()
}

fn validate_stored_object(
    repo_id: &str,
    object: &CanonicalObject<Value>,
) -> Result<(), RelayError> {
    validate_stored_object_with_policy(
        repo_id,
        object,
        signature_enforcement_enabled(),
        &legacy_unsigned_repo_ids(),
    )
}

fn validate_stored_object_with_policy(
    repo_id: &str,
    object: &CanonicalObject<Value>,
    enforce_signatures: bool,
    legacy_unsigned_repos: &BTreeSet<String>,
) -> Result<(), RelayError> {
    let is_legacy_unsigned_repo = legacy_unsigned_repos.contains(repo_id);
    validate_object_with_signature_policy(
        object,
        enforce_signatures && !is_legacy_unsigned_repo,
        enforce_signatures,
    )
}

fn validate_object_with_policy(
    object: &CanonicalObject<Value>,
    enforce_signatures: bool,
) -> Result<(), RelayError> {
    validate_object_with_signature_policy(object, enforce_signatures, enforce_signatures)
}

fn validate_object_with_signature_policy(
    object: &CanonicalObject<Value>,
    require_signatures: bool,
    verify_present_signatures: bool,
) -> Result<(), RelayError> {
    let computed_body_hash = stable_json_hash(&object.body)?;
    if object.body_hash != computed_body_hash {
        return Err(RelayError::InvalidObject {
            object_id: object.object_id.clone(),
            message: format!(
                "body_hash mismatch: found {}, computed {}",
                object.body_hash, computed_body_hash
            ),
        });
    }
    let computed_object_id =
        canonical_object_id(&object.kind, &object.schema_version, &object.body)?;
    if object.object_id != computed_object_id {
        return Err(RelayError::InvalidObject {
            object_id: object.object_id.clone(),
            message: format!(
                "object_id mismatch: found {}, computed {}",
                object.object_id, computed_object_id
            ),
        });
    }
    if require_signatures && object.signatures.is_empty() {
        return Err(RelayError::InvalidObject {
            object_id: object.object_id.clone(),
            message: "signature enforcement enabled but object carries no signatures".to_string(),
        });
    }
    if verify_present_signatures && !object.signatures.is_empty() {
        for signature in &object.signatures {
            let ok = verify_signature(
                &object.kind,
                &object.schema_version,
                &object.body,
                signature,
            )
            .map_err(|err| RelayError::InvalidObject {
                object_id: object.object_id.clone(),
                message: format!("signature verification error: {err}"),
            })?;
            if !ok {
                return Err(RelayError::InvalidObject {
                    object_id: object.object_id.clone(),
                    message: format!("invalid signature for public_key {}", signature.public_key),
                });
            }
        }
    }
    Ok(())
}

fn canonical_object_bytes(object: &CanonicalObject<Value>) -> Result<Vec<u8>, RelayError> {
    let value = serde_json::to_value(object)?;
    Ok(serde_json::to_vec(&canonical_json(&value))?)
}

fn bundle_hash(object_ids: &[String]) -> Result<String, RelayError> {
    Ok(stable_json_hash(
        &serde_json::json!({ "object_ids": object_ids }),
    )?)
}

fn key_hash(key: &str) -> String {
    let digest = Sha256::digest(key.as_bytes());
    hex_lower(&digest)
}

fn hex_lower(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn read_json<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<T, RelayError> {
    Ok(serde_json::from_slice(&std::fs::read(path)?)?)
}

fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<(), RelayError> {
    let bytes = serde_json::to_vec_pretty(value)?;
    write_bytes(path, &bytes)
}

fn write_bytes(path: &Path, bytes: &[u8]) -> Result<(), RelayError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, bytes)?;
    Ok(())
}

pub fn kfdb_index_batch(
    repo_id: &str,
    bundle_hash_value: &str,
    objects: &[CanonicalObject<Value>],
) -> Result<KfdbWriteBatch, RelayError> {
    let repo_node_id = deterministic_uuid(&["RickydataRepository", repo_id]);
    let bundle_node_id = deterministic_uuid(&["RickydataBundle", repo_id, bundle_hash_value]);
    let mut operations = vec![
        create_node(
            &repo_node_id,
            "RickydataRepository",
            vec![
                ("repo_id", string_value(repo_id)),
                ("schema_version", string_value("rickydata.git.kfdb.v1")),
            ],
        ),
        create_node(
            &bundle_node_id,
            "RickydataBundle",
            vec![
                ("repo_id", string_value(repo_id)),
                ("bundle_hash", string_value(bundle_hash_value)),
                ("object_count", integer_value(objects.len() as i64)),
                ("schema_version", string_value("rickydata.git.kfdb.v1")),
            ],
        ),
        create_edge(
            &deterministic_uuid(&["HAS_RICKYDATA_BUNDLE", &repo_node_id, &bundle_node_id]),
            &repo_node_id,
            &bundle_node_id,
            "HAS_RICKYDATA_BUNDLE",
            vec![("source", string_value("rickydata-git-relay"))],
        ),
    ];

    for object in objects {
        validate_object_with_policy(object, false)?;
        let object_node_id =
            deterministic_uuid(&["RickydataObjectMirror", repo_id, &object.object_id]);
        let canonical_byte_hash = hex_lower(&Sha256::digest(canonical_object_bytes(object)?));
        operations.push(create_node(
            &object_node_id,
            "RickydataObjectMirror",
            vec![
                ("repo_id", string_value(repo_id)),
                ("object_id", string_value(&object.object_id)),
                ("kind", string_value(&object.kind)),
                ("schema_version", string_value(&object.schema_version)),
                ("body_hash", string_value(&object.body_hash)),
                (
                    "canonical_byte_hash",
                    string_value(&format!("sha256:{canonical_byte_hash}")),
                ),
                ("bundle_hash", string_value(bundle_hash_value)),
            ],
        ));
        operations.push(create_edge(
            &deterministic_uuid(&["HAS_RICKYDATA_OBJECT", &repo_node_id, &object_node_id]),
            &repo_node_id,
            &object_node_id,
            "HAS_RICKYDATA_OBJECT",
            vec![("object_id", string_value(&object.object_id))],
        ));
        operations.push(create_edge(
            &deterministic_uuid(&["BUNDLE_CONTAINS_OBJECT", &bundle_node_id, &object_node_id]),
            &bundle_node_id,
            &object_node_id,
            "BUNDLE_CONTAINS_OBJECT",
            vec![("object_id", string_value(&object.object_id))],
        ));
        append_signature_operations(object, &object_node_id, &mut operations);
        append_projection_operations(repo_id, object, &object_node_id, &mut operations);
        append_provenance_operations(repo_id, object, &mut operations);
    }

    Ok(KfdbWriteBatch {
        operations,
        skip_embedding: true,
    })
}

fn append_projection_operations(
    repo_id: &str,
    object: &CanonicalObject<Value>,
    object_node_id: &str,
    operations: &mut Vec<Value>,
) {
    let Some((label, edge_type)) = projection_label_and_edge(&object.kind) else {
        return;
    };
    let projection_key = projection_key(object);
    let projection_node_id = deterministic_uuid(&[label, repo_id, &projection_key]);
    let mut properties = vec![
        ("repo_id", string_value(repo_id)),
        ("object_id", string_value(&object.object_id)),
        ("kind", string_value(&object.kind)),
        ("body_hash", string_value(&object.body_hash)),
        ("projection_key", string_value(&projection_key)),
        ("schema_version", string_value("rickydata.git.kfdb.v1")),
    ];
    append_agent_projection_properties(object, &mut properties);
    operations.push(create_node(&projection_node_id, label, properties));
    operations.push(create_edge(
        &deterministic_uuid(&[edge_type, object_node_id, &projection_node_id]),
        object_node_id,
        &projection_node_id,
        edge_type,
        vec![("object_id", string_value(&object.object_id))],
    ));

    if object.kind == "agent.change" || object.kind == "agent.patch" {
        for file_path in object
            .body
            .get("file_paths")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
        {
            let file_node_id = deterministic_uuid(&["CodeFile", repo_id, file_path]);
            operations.push(create_node(
                &file_node_id,
                "CodeFile",
                vec![
                    ("repo_id", string_value(repo_id)),
                    ("path", string_value(file_path)),
                    ("path_hash", string_value(&key_hash(file_path))),
                ],
            ));
            operations.push(create_edge(
                &deterministic_uuid(&["TOUCHES_FILE", &projection_node_id, &file_node_id]),
                &projection_node_id,
                &file_node_id,
                "TOUCHES_FILE",
                vec![("source", string_value("rickydata-git-relay"))],
            ));
        }
    }
}

fn append_agent_projection_properties(
    object: &CanonicalObject<Value>,
    properties: &mut Vec<(&'static str, Value)>,
) {
    for field in [
        "intent_id",
        "attempt_id",
        "run_id",
        "change_id",
        "patch_id",
        "base_commit",
        "diff_hash",
    ] {
        if let Some(value) = object.body.get(field).and_then(Value::as_str) {
            properties.push((field, string_value(value)));
        }
    }
    for field in ["run_ids", "change_ids", "file_paths", "diff_hashes"] {
        if let Some(value) = object.body.get(field) {
            properties.push((field, string_value(&value.to_string())));
        }
    }
    if let Some(value) = object.body.get("diff_summary") {
        properties.push(("diff_summary", string_value(&value.to_string())));
    }
}

fn append_signature_operations(
    object: &CanonicalObject<Value>,
    object_node_id: &str,
    operations: &mut Vec<Value>,
) {
    for signature in &object.signatures {
        let actor_node_id = deterministic_uuid(&["RickydataActor", &signature.public_key]);
        let mut actor_properties = vec![
            ("public_key", string_value(&signature.public_key)),
            ("algorithm", string_value(&signature.algorithm)),
            ("schema_version", string_value("rickydata.git.kfdb.v1")),
        ];
        if let Some(label) = &signature.signer_label {
            actor_properties.push(("signer_label", string_value(label)));
        }
        operations.push(create_node(
            &actor_node_id,
            "RickydataActor",
            actor_properties,
        ));
        let mut edge_properties = vec![
            ("object_id", string_value(&object.object_id)),
            ("algorithm", string_value(&signature.algorithm)),
            ("signature", string_value(&signature.signature)),
        ];
        if let Some(label) = &signature.signer_label {
            edge_properties.push(("signer_label", string_value(label)));
        }
        if let Some(signed_at_ms) = signature.signed_at_ms {
            edge_properties.push(("signed_at_ms", integer_value(signed_at_ms as i64)));
        }
        operations.push(create_edge(
            &deterministic_uuid(&[
                "SIGNED_BY",
                object_node_id,
                &actor_node_id,
                &signature.signature,
            ]),
            object_node_id,
            &actor_node_id,
            "SIGNED_BY",
            edge_properties,
        ));
    }
}

fn append_provenance_operations(
    repo_id: &str,
    object: &CanonicalObject<Value>,
    operations: &mut Vec<Value>,
) {
    match object.kind.as_str() {
        "agent.attempt" => {
            if let Some(intent_id) = body_string(object, "intent_id") {
                emit_lineage_edge(
                    repo_id,
                    operations,
                    ("RickydataAgentAttempt", "attempt_id", object),
                    ("RickydataWorkIntent", &intent_id, "objective"),
                    "INTENT_CHAIN",
                );
            }
        }
        "agent.run" => {
            if let Some(attempt_id) = body_string(object, "attempt_id") {
                emit_lineage_edge(
                    repo_id,
                    operations,
                    ("RickydataAgentRun", "run_id", object),
                    ("RickydataAgentAttempt", &attempt_id, "attempt_id"),
                    "ATTEMPT_LINEAGE",
                );
            }
        }
        "agent.change" => {
            for run_id in body_string_array(object, "run_ids") {
                emit_lineage_edge(
                    repo_id,
                    operations,
                    ("RickydataChangeEvidence", "change_id", object),
                    ("RickydataAgentRun", &run_id, "run_id"),
                    "RUN_LINEAGE",
                );
            }
            if let Some(run_id) = body_string(object, "run_id") {
                emit_lineage_edge(
                    repo_id,
                    operations,
                    ("RickydataChangeEvidence", "change_id", object),
                    ("RickydataAgentRun", &run_id, "run_id"),
                    "RUN_LINEAGE",
                );
            }
        }
        "agent.patch" => {
            for change_id in body_string_array(object, "change_ids") {
                emit_lineage_edge(
                    repo_id,
                    operations,
                    ("RickydataPreparedPatch", "patch_id", object),
                    ("RickydataChangeEvidence", &change_id, "change_id"),
                    "PATCH_LINEAGE",
                );
            }
            if let Some(change_id) = body_string(object, "change_id") {
                emit_lineage_edge(
                    repo_id,
                    operations,
                    ("RickydataPreparedPatch", "patch_id", object),
                    ("RickydataChangeEvidence", &change_id, "change_id"),
                    "PATCH_LINEAGE",
                );
            }
        }
        _ => {}
    }
}

fn emit_lineage_edge(
    repo_id: &str,
    operations: &mut Vec<Value>,
    source: (&str, &str, &CanonicalObject<Value>),
    target: (&str, &str, &str),
    edge_type: &str,
) {
    let (source_label, source_key_field, source_object) = source;
    let (target_label, target_projection_key, target_key_field) = target;
    let source_projection_key = body_string(source_object, source_key_field)
        .unwrap_or_else(|| source_object.object_id.clone());
    let source_node_id = deterministic_uuid(&[source_label, repo_id, &source_projection_key]);
    let target_node_id = deterministic_uuid(&[target_label, repo_id, target_projection_key]);
    operations.push(create_node(
        &target_node_id,
        target_label,
        vec![
            ("repo_id", string_value(repo_id)),
            (target_key_field, string_value(target_projection_key)),
            ("projection_key", string_value(target_projection_key)),
            ("schema_version", string_value("rickydata.git.kfdb.v1")),
        ],
    ));
    operations.push(create_edge(
        &deterministic_uuid(&[edge_type, &source_node_id, &target_node_id]),
        &source_node_id,
        &target_node_id,
        edge_type,
        vec![
            ("source_object_id", string_value(&source_object.object_id)),
            ("source_kind", string_value(&source_object.kind)),
        ],
    ));
}

fn body_string(object: &CanonicalObject<Value>, field: &str) -> Option<String> {
    object
        .body
        .get(field)
        .and_then(Value::as_str)
        .map(str::to_string)
}

fn body_string_array(object: &CanonicalObject<Value>, field: &str) -> Vec<String> {
    object
        .body
        .get(field)
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::to_string)
        .collect()
}

fn projection_label_and_edge(kind: &str) -> Option<(&'static str, &'static str)> {
    match kind {
        "agent.intent" => Some(("RickydataWorkIntent", "HAS_WORK_INTENT")),
        "agent.attempt" => Some(("RickydataAgentAttempt", "HAS_AGENT_ATTEMPT")),
        "agent.attempt_status" => Some(("RickydataAttemptStatusTransition", "HAS_ATTEMPT_STATUS")),
        "agent.run" => Some(("RickydataAgentRun", "HAS_AGENT_RUN")),
        "agent.run_trace" => Some(("RickydataAgentRunTrace", "HAS_AGENT_RUN_TRACE")),
        "agent.change" => Some(("RickydataChangeEvidence", "HAS_CHANGE_EVIDENCE")),
        "agent.patch" => Some(("RickydataPreparedPatch", "HAS_PREPARED_PATCH")),
        "agent.patch_retirement" => Some(("RickydataPatchRetirement", "SUPERSEDES_PATCH")),
        _ => None,
    }
}

fn projection_key(object: &CanonicalObject<Value>) -> String {
    let fields: &[&str] = match object.kind.as_str() {
        "agent.intent" => &["objective"],
        "agent.attempt" | "agent.attempt_status" => &["attempt_id"],
        "agent.run" => &["run_id"],
        "agent.run_trace" => &["trace_id", "run_id", "attempt_id"],
        "agent.change" => &["change_id"],
        "agent.patch" | "agent.patch_retirement" => &["patch_id"],
        _ => &[
            "trace_id",
            "attempt_id",
            "run_id",
            "change_id",
            "patch_id",
            "objective",
        ],
    };
    for field in fields {
        if let Some(value) = object.body.get(field).and_then(Value::as_str) {
            return value.to_string();
        }
    }
    object.object_id.clone()
}

fn deterministic_uuid(parts: &[&str]) -> String {
    Uuid::new_v5(&Uuid::NAMESPACE_URL, parts.join("\u{1f}").as_bytes()).to_string()
}

fn create_node(id: &str, label: &str, properties: Vec<(&str, Value)>) -> Value {
    serde_json::json!({
        "operation": "create_node",
        "id": id,
        "label": label,
        "mode": "merge",
        "properties": properties_object(properties),
    })
}

fn create_edge(
    id: &str,
    from: &str,
    to: &str,
    edge_type: &str,
    properties: Vec<(&str, Value)>,
) -> Value {
    serde_json::json!({
        "operation": "create_edge",
        "id": id,
        "from": from,
        "to": to,
        "edge_type": edge_type,
        "properties": properties_object(properties),
    })
}

fn properties_object(properties: Vec<(&str, Value)>) -> Value {
    let mut map = serde_json::Map::new();
    for (key, value) in properties {
        map.insert(key.to_string(), value);
    }
    Value::Object(map)
}

fn string_value(value: &str) -> Value {
    serde_json::json!({ "String": value })
}

fn integer_value(value: i64) -> Value {
    serde_json::json!({ "Integer": value })
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::{Body, to_bytes};
    use axum::extract::State;
    use axum::extract::{Path as TestPath, Query};
    use axum::http::{HeaderMap, Method, Request, StatusCode};
    use axum::response::IntoResponse;
    use axum::routing::post;
    use axum::{Json, Router};
    use percent_encoding::percent_decode_str;
    use rickydata_git_core::{CanonicalObject, DEFAULT_SCHEMA_VERSION};
    use std::collections::BTreeMap;
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tokio::sync::oneshot;
    use tower::ServiceExt;

    fn object(body: Value) -> CanonicalObject<Value> {
        CanonicalObject::new("agent.intent", DEFAULT_SCHEMA_VERSION, 123, body).unwrap()
    }

    async fn json_response<T: for<'de> Deserialize<'de>>(response: Response) -> T {
        let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        serde_json::from_slice(&bytes).unwrap()
    }

    #[test]
    fn validates_and_pushes_content_addressed_objects() {
        let temp = tempfile::tempdir().unwrap();
        let store = FileRelayStore::new(temp.path());
        let first = object(serde_json::json!({ "issue": "local:1", "objective": "test" }));
        let request = BundlePushRequest {
            repo_id: "rickydata_code".to_string(),
            idempotency_key: "first".to_string(),
            objects: vec![first.clone()],
        };

        let validation = store.validate_bundle(&request).unwrap();
        let push = store.push_bundle(&request).unwrap();
        let pull = store
            .pull_bundle(&BundlePullRequest {
                repo_id: "rickydata_code".to_string(),
                known_object_ids: Vec::new(),
                limit: None,
            })
            .unwrap();

        assert_eq!(validation.status, "ok");
        assert_eq!(push.accepted_object_count, 1);
        assert_eq!(push.duplicate_object_count, 0);
        assert_eq!(pull.object_count, 1);
        assert_eq!(pull.objects[0], first);
    }

    #[test]
    fn repeated_idempotency_key_returns_original_report() {
        let temp = tempfile::tempdir().unwrap();
        let store = FileRelayStore::new(temp.path());
        let first = object(serde_json::json!({ "issue": "local:1" }));
        let second = object(serde_json::json!({ "issue": "local:2" }));
        let request = BundlePushRequest {
            repo_id: "repo".to_string(),
            idempotency_key: "same-key".to_string(),
            objects: vec![first],
        };
        let replay = BundlePushRequest {
            objects: vec![second],
            ..request.clone()
        };

        let pushed = store.push_bundle(&request).unwrap();
        let replayed = store.push_bundle(&replay).unwrap();

        assert_eq!(replayed, pushed);
    }

    #[test]
    fn pull_skips_known_objects_and_respects_limit() {
        let temp = tempfile::tempdir().unwrap();
        let store = FileRelayStore::new(temp.path());
        let first = object(serde_json::json!({ "issue": "local:1" }));
        let second = object(serde_json::json!({ "issue": "local:2" }));
        store
            .push_bundle(&BundlePushRequest {
                repo_id: "repo".to_string(),
                idempotency_key: "push".to_string(),
                objects: vec![first.clone(), second],
            })
            .unwrap();

        let pull = store
            .pull_bundle(&BundlePullRequest {
                repo_id: "repo".to_string(),
                known_object_ids: vec![first.object_id],
                limit: Some(1),
            })
            .unwrap();

        assert_eq!(pull.object_count, 1);
        assert_eq!(pull.remaining_object_count, 0);
    }

    #[test]
    fn rejects_objects_with_mismatched_hashes() {
        let temp = tempfile::tempdir().unwrap();
        let store = FileRelayStore::new(temp.path());
        let mut bad = object(serde_json::json!({ "issue": "local:1" }));
        bad.body_hash =
            "sha256:0000000000000000000000000000000000000000000000000000000000000000".to_string();

        let error = store
            .push_bundle(&BundlePushRequest {
                repo_id: "repo".to_string(),
                idempotency_key: "bad".to_string(),
                objects: vec![bad],
            })
            .unwrap_err();

        assert!(error.to_string().contains("body_hash mismatch"));
    }

    #[derive(Default)]
    struct RecordingIndexSink {
        batches: Mutex<Vec<KfdbWriteBatch>>,
    }

    impl KfdbIndexSink for RecordingIndexSink {
        fn write_batch(&self, batch: &KfdbWriteBatch) -> Result<(), RelayError> {
            self.batches.lock().unwrap().push(batch.clone());
            Ok(())
        }
    }

    struct FailingIndexSink;

    impl KfdbIndexSink for FailingIndexSink {
        fn write_batch(&self, _batch: &KfdbWriteBatch) -> Result<(), RelayError> {
            Err(RelayError::KfdbIndex("simulated KFDB outage".to_string()))
        }
    }

    struct StatusOnlyStore;

    impl RelayStore for StatusOnlyStore {
        fn validate_bundle(
            &self,
            _request: &BundlePushRequest,
        ) -> Result<BundleValidationReport, RelayError> {
            unimplemented!("not needed for status delegation test")
        }

        fn push_bundle(
            &self,
            _request: &BundlePushRequest,
        ) -> Result<BundlePushReport, RelayError> {
            unimplemented!("not needed for status delegation test")
        }

        fn pull_bundle(
            &self,
            _request: &BundlePullRequest,
        ) -> Result<BundlePullReport, RelayError> {
            unimplemented!("not needed for status delegation test")
        }

        fn read_object(
            &self,
            _repo_id: &str,
            _object_id: &str,
        ) -> Result<CanonicalObject<Value>, RelayError> {
            unimplemented!("not needed for status delegation test")
        }

        fn list_object_ids(&self, _repo_id: &str) -> Result<Vec<String>, RelayError> {
            panic!("IndexedRelayStore must delegate repo_status instead of using the default")
        }

        fn repo_status(&self, repo_id: &str) -> Result<RepoRelayStatusReport, RelayError> {
            Ok(RepoRelayStatusReport {
                status: "ok".to_string(),
                repo_id: repo_id.to_string(),
                object_count: 7,
                object_ids_hash: "sha256:delegated".to_string(),
            })
        }
    }

    #[test]
    fn indexed_store_delegates_repo_status_to_inner_store() {
        let store = IndexedRelayStore::new(StatusOnlyStore, RecordingIndexSink::default());

        let status = store.repo_status("repo").unwrap();

        assert_eq!(status.object_count, 7);
        assert_eq!(status.object_ids_hash, "sha256:delegated");
    }

    #[test]
    fn indexed_store_writes_kfdb_batch_after_bundle_persistence() {
        let temp = tempfile::tempdir().unwrap();
        let store = IndexedRelayStore::new(
            FileRelayStore::new(temp.path()),
            RecordingIndexSink::default(),
        );
        let first = object(serde_json::json!({ "objective": "index me" }));
        let request = BundlePushRequest {
            repo_id: "repo".to_string(),
            idempotency_key: "index".to_string(),
            objects: vec![first.clone()],
        };

        let report = store.push_bundle(&request).unwrap();
        let fetched = store.read_object("repo", &first.object_id).unwrap();
        let batches = store.index_sink.batches.lock().unwrap();

        assert_eq!(report.accepted_object_count, 1);
        assert_eq!(fetched, first);
        assert_eq!(batches.len(), 1);
        assert!(batches[0].skip_embedding);
        assert_eq!(batches[0].operations[0]["operation"], "create_node");
    }

    #[test]
    fn indexed_store_keeps_canonical_push_success_when_kfdb_projection_fails() {
        let temp = tempfile::tempdir().unwrap();
        let store = IndexedRelayStore::new(FileRelayStore::new(temp.path()), FailingIndexSink);
        let first = object(serde_json::json!({ "objective": "persist despite KFDB outage" }));
        let request = BundlePushRequest {
            repo_id: "repo".to_string(),
            idempotency_key: "kfdb-outage".to_string(),
            objects: vec![first.clone()],
        };

        let report = store.push_bundle(&request).unwrap();
        let fetched = store.read_object("repo", &first.object_id).unwrap();

        assert_eq!(report.status, "ok");
        assert_eq!(report.accepted_object_count, 1);
        assert_eq!(fetched, first);
    }

    #[test]
    fn indexed_store_chunks_large_kfdb_projection_batches() {
        let temp = tempfile::tempdir().unwrap();
        let store = IndexedRelayStore::new(
            FileRelayStore::new(temp.path()),
            RecordingIndexSink::default(),
        );
        let objects = (0..40)
            .map(|index| object(serde_json::json!({ "objective": format!("index {index}") })))
            .collect::<Vec<_>>();
        let request = BundlePushRequest {
            repo_id: "repo".to_string(),
            idempotency_key: "chunk-index".to_string(),
            objects,
        };

        let report = store.push_bundle(&request).unwrap();
        let batches = store.index_sink.batches.lock().unwrap();

        assert_eq!(report.accepted_object_count, 40);
        assert!(batches.len() > 1, "large projections must be chunked");
        assert!(batches.iter().all(|batch| batch.skip_embedding
            && batch.operations.len() <= KFDB_INDEX_BATCH_OPERATION_LIMIT));
        assert!(batches.iter().all(|batch| !batch.operations.is_empty()));
    }

    #[derive(Default)]
    struct MockGcsState {
        objects: Mutex<BTreeMap<String, Vec<u8>>>,
        token_requests: AtomicUsize,
        upload_requests: AtomicUsize,
        download_requests: AtomicUsize,
    }

    #[derive(Deserialize)]
    struct MockGcsUploadQuery {
        name: String,
    }

    #[derive(Deserialize)]
    struct MockGcsListQuery {
        prefix: Option<String>,
    }

    async fn mock_gcs_upload(
        State(state): State<Arc<MockGcsState>>,
        headers: HeaderMap,
        Query(query): Query<MockGcsUploadQuery>,
        body: Body,
    ) -> StatusCode {
        if !mock_gcs_authorized(&headers) {
            return StatusCode::UNAUTHORIZED;
        }
        state.upload_requests.fetch_add(1, Ordering::SeqCst);
        let bytes = to_bytes(body, usize::MAX).await.unwrap().to_vec();
        let mut objects = state.objects.lock().unwrap();
        if objects.contains_key(&query.name) {
            return StatusCode::PRECONDITION_FAILED;
        }
        objects.insert(query.name, bytes);
        StatusCode::OK
    }

    async fn mock_gcs_get(
        State(state): State<Arc<MockGcsState>>,
        headers: HeaderMap,
        TestPath((_bucket, encoded_name)): TestPath<(String, String)>,
    ) -> Response {
        if !mock_gcs_authorized(&headers) {
            return StatusCode::UNAUTHORIZED.into_response();
        }
        state.download_requests.fetch_add(1, Ordering::SeqCst);
        let name = percent_decode_str(&encoded_name)
            .decode_utf8_lossy()
            .to_string();
        let objects = state.objects.lock().unwrap();
        match objects.get(&name) {
            Some(bytes) => (StatusCode::OK, bytes.clone()).into_response(),
            None => StatusCode::NOT_FOUND.into_response(),
        }
    }

    async fn mock_gcs_list(
        State(state): State<Arc<MockGcsState>>,
        headers: HeaderMap,
        Query(query): Query<MockGcsListQuery>,
    ) -> Response {
        if !mock_gcs_authorized(&headers) {
            return StatusCode::UNAUTHORIZED.into_response();
        }
        let prefix = query.prefix.unwrap_or_default();
        let objects = state.objects.lock().unwrap();
        let items = objects
            .keys()
            .filter(|name| name.starts_with(&prefix))
            .map(|name| serde_json::json!({ "name": name }))
            .collect::<Vec<_>>();
        Json(serde_json::json!({ "items": items })).into_response()
    }

    async fn mock_metadata_token(
        State(state): State<Arc<MockGcsState>>,
        headers: HeaderMap,
    ) -> Response {
        if headers
            .get("Metadata-Flavor")
            .and_then(|value| value.to_str().ok())
            != Some("Google")
        {
            return StatusCode::FORBIDDEN.into_response();
        }
        state.token_requests.fetch_add(1, Ordering::SeqCst);
        Json(serde_json::json!({
            "access_token": "mock-gcs-token",
            "expires_in": 3600,
            "token_type": "Bearer"
        }))
        .into_response()
    }

    fn mock_gcs_authorized(headers: &HeaderMap) -> bool {
        headers
            .get("authorization")
            .and_then(|value| value.to_str().ok())
            == Some("Bearer mock-gcs-token")
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn gcs_store_push_pull_round_trip_against_mock_api() {
        let state = Arc::new(MockGcsState::default());
        let app = Router::new()
            .route("/upload/storage/v1/b/{bucket}/o", post(mock_gcs_upload))
            .route("/storage/v1/b/{bucket}/o", get(mock_gcs_list))
            .route("/storage/v1/b/{bucket}/o/{*name}", get(mock_gcs_get))
            .route(
                "/computeMetadata/v1/instance/service-accounts/default/token",
                get(mock_metadata_token),
            )
            .with_state(state.clone());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
        let server = tokio::spawn(async move {
            axum::serve(listener, app)
                .with_graceful_shutdown(async {
                    let _ = shutdown_rx.await;
                })
                .await
                .unwrap();
        });
        let base_url = format!("http://{addr}");
        let first = object(serde_json::json!({ "objective": "gcs durable relay" }));
        let second = object(serde_json::json!({ "objective": "gcs token cache" }));

        let state_for_assertion = state.clone();
        let (pushed, replayed, pulled, fetched, status) = tokio::task::spawn_blocking(move || {
            let store = GcsRelayStore::with_base_urls(
                "bucket",
                &base_url,
                &base_url,
                GcsAuth::Metadata {
                    token_url: format!(
                        "{base_url}/computeMetadata/v1/instance/service-accounts/default/token"
                    ),
                },
            )?;
            let request = BundlePushRequest {
                repo_id: "repo".to_string(),
                idempotency_key: "gcs-first".to_string(),
                objects: vec![first.clone(), second.clone()],
            };
            let pushed = store.push_bundle(&request)?;
            let replayed = store.push_bundle(&request)?;
            let pulled = store.pull_bundle(&BundlePullRequest {
                repo_id: "repo".to_string(),
                known_object_ids: Vec::new(),
                limit: None,
            })?;
            let fetched = store.read_object("repo", &first.object_id)?;
            let status = store.repo_status("repo")?;
            Ok::<_, RelayError>((pushed, replayed, pulled, fetched, status))
        })
        .await
        .unwrap()
        .unwrap();

        assert_eq!(pushed.accepted_object_count, 2);
        assert_eq!(pushed.duplicate_object_count, 0);
        assert_eq!(replayed, pushed);
        assert_eq!(pulled.object_count, 2);
        assert!(pulled.objects.contains(&fetched));
        assert_eq!(status.object_count, 2);
        assert_eq!(state_for_assertion.token_requests.load(Ordering::SeqCst), 1);
        assert_eq!(
            state_for_assertion.upload_requests.load(Ordering::SeqCst),
            3
        );
        state_for_assertion
            .download_requests
            .store(0, Ordering::SeqCst);
        let status_after_reset = tokio::task::spawn_blocking({
            let base_url = format!("http://{addr}");
            move || {
                let store = GcsRelayStore::with_base_urls(
                    "bucket",
                    &base_url,
                    &base_url,
                    GcsAuth::Metadata {
                        token_url: format!(
                            "{base_url}/computeMetadata/v1/instance/service-accounts/default/token"
                        ),
                    },
                )?;
                store.repo_status("repo")
            }
        })
        .await
        .unwrap()
        .unwrap();
        assert_eq!(status_after_reset.object_count, 2);
        assert_eq!(
            state_for_assertion.download_requests.load(Ordering::SeqCst),
            0
        );
        let limited = tokio::task::spawn_blocking({
            let base_url = format!("http://{addr}");
            move || {
                let store = GcsRelayStore::with_base_urls(
                    "bucket",
                    &base_url,
                    &base_url,
                    GcsAuth::Metadata {
                        token_url: format!(
                            "{base_url}/computeMetadata/v1/instance/service-accounts/default/token"
                        ),
                    },
                )?;
                store.pull_bundle(&BundlePullRequest {
                    repo_id: "repo".to_string(),
                    known_object_ids: Vec::new(),
                    limit: Some(1),
                })
            }
        })
        .await
        .unwrap()
        .unwrap();
        assert_eq!(limited.object_count, 1);
        assert_eq!(limited.remaining_object_count, 1);
        assert_eq!(
            state_for_assertion.download_requests.load(Ordering::SeqCst),
            1
        );
        shutdown_tx.send(()).unwrap();
        server.await.unwrap();
    }

    struct CapturedKfdbRequest {
        authorization: Option<String>,
        derive_session_id: Option<String>,
        derive_key: Option<String>,
        wallet_address: Option<String>,
        body: Value,
    }

    async fn capture_kfdb_write(
        State(captured): State<Arc<Mutex<Vec<CapturedKfdbRequest>>>>,
        headers: HeaderMap,
        Json(body): Json<Value>,
    ) -> StatusCode {
        captured.lock().unwrap().push(CapturedKfdbRequest {
            authorization: headers
                .get("authorization")
                .and_then(|value| value.to_str().ok())
                .map(ToOwned::to_owned),
            derive_session_id: headers
                .get("x-derive-session-id")
                .and_then(|value| value.to_str().ok())
                .map(ToOwned::to_owned),
            derive_key: headers
                .get("x-derive-key")
                .and_then(|value| value.to_str().ok())
                .map(ToOwned::to_owned),
            wallet_address: headers
                .get("x-wallet-address")
                .and_then(|value| value.to_str().ok())
                .map(ToOwned::to_owned),
            body,
        });
        StatusCode::OK
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn http_kfdb_index_sink_posts_write_batch_with_auth() {
        let captured = Arc::new(Mutex::new(Vec::new()));
        let app = Router::new()
            .route("/api/v1/write", post(capture_kfdb_write))
            .with_state(captured.clone());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
        let server = tokio::spawn(async move {
            axum::serve(listener, app)
                .with_graceful_shutdown(async {
                    let _ = shutdown_rx.await;
                })
                .await
                .unwrap();
        });

        let base_url = format!("http://{addr}");
        let batch = KfdbWriteBatch {
            operations: vec![serde_json::json!({
                "operation": "create_node",
                "label": "RickydataObjectMirror"
            })],
            skip_embedding: true,
        };

        tokio::task::spawn_blocking(move || {
            let sink = HttpKfdbIndexSink::new(
                base_url,
                Some("test-token".to_string()),
                KfdbPrivateAuth {
                    derive_session_id: "derive-session".to_string(),
                    derive_key: "00".repeat(32),
                    wallet_address: Some("0xabc".to_string()),
                },
            )?;
            sink.write_batch(&batch)
        })
        .await
        .unwrap()
        .unwrap();

        let (
            request_count,
            authorization,
            derive_session_id,
            derive_key,
            wallet_address,
            skip_embedding,
            label,
        ) = {
            let requests = captured.lock().unwrap();
            (
                requests.len(),
                requests[0].authorization.clone(),
                requests[0].derive_session_id.clone(),
                requests[0].derive_key.clone(),
                requests[0].wallet_address.clone(),
                requests[0].body["skip_embedding"].clone(),
                requests[0].body["operations"][0]["label"].clone(),
            )
        };
        assert_eq!(request_count, 1);
        assert_eq!(authorization.as_deref(), Some("Bearer test-token"));
        assert_eq!(derive_session_id.as_deref(), Some("derive-session"));
        assert_eq!(
            derive_key.as_deref(),
            Some("0000000000000000000000000000000000000000000000000000000000000000")
        );
        assert_eq!(wallet_address.as_deref(), Some("0xabc"));
        assert_eq!(skip_embedding, true);
        assert_eq!(label, "RickydataObjectMirror");
        shutdown_tx.send(()).unwrap();
        server.await.unwrap();
    }

    #[test]
    fn kfdb_index_batch_emits_mirror_projection_and_skip_embedding() {
        let intent = object(serde_json::json!({
            "objective": "Use rickygit",
            "issue_refs": [{"platform": "github", "repository": "ricky/repo", "id": "1"}],
            "task_refs": [],
            "privacy": "public_metadata"
        }));
        let change = CanonicalObject::new(
            "agent.change",
            DEFAULT_SCHEMA_VERSION,
            123,
            serde_json::json!({
                "change_id": "sha256:change",
                "attempt_id": "sha256:attempt",
                "file_paths": ["src/lib.rs"]
            }),
        )
        .unwrap();
        let trace = CanonicalObject::new(
            "agent.run_trace",
            DEFAULT_SCHEMA_VERSION,
            123,
            serde_json::json!({
                "trace_id": "sha256:trace",
                "attempt_id": "sha256:attempt",
                "command_hash": "sha256:command"
            }),
        )
        .unwrap();

        let first = kfdb_index_batch(
            "repo",
            "sha256:bundle",
            &[intent.clone(), change.clone(), trace.clone()],
        )
        .unwrap();
        let second = kfdb_index_batch("repo", "sha256:bundle", &[intent, change, trace]).unwrap();
        let labels = first
            .operations
            .iter()
            .filter_map(|operation| operation.get("label").and_then(Value::as_str))
            .collect::<BTreeSet<_>>();
        let edge_types = first
            .operations
            .iter()
            .filter_map(|operation| operation.get("edge_type").and_then(Value::as_str))
            .collect::<BTreeSet<_>>();

        assert_eq!(first, second);
        assert!(first.skip_embedding);
        assert!(labels.contains("RickydataObjectMirror"));
        assert!(labels.contains("RickydataWorkIntent"));
        assert!(labels.contains("RickydataAgentRunTrace"));
        assert!(labels.contains("RickydataChangeEvidence"));
        assert!(labels.contains("CodeFile"));
        assert!(edge_types.contains("HAS_RICKYDATA_OBJECT"));
        assert!(edge_types.contains("HAS_WORK_INTENT"));
        assert!(edge_types.contains("HAS_AGENT_RUN_TRACE"));
        assert!(edge_types.contains("HAS_CHANGE_EVIDENCE"));
        assert!(edge_types.contains("TOUCHES_FILE"));
        assert!(
            serde_json::to_string(&first)
                .unwrap()
                .contains("\"skip_embedding\":true")
        );
        assert!(!serde_json::to_string(&first).unwrap().contains("\"body\""));
    }

    #[test]
    fn kfdb_index_batch_emits_actor_nodes_and_signed_by_edges_for_signed_objects() {
        use rickydata_git_core::{generate_signing_keypair, sign_object};

        let key = generate_signing_keypair();
        let mut intent = object(serde_json::json!({
            "objective": "Signed intent",
            "issue_refs": [],
            "task_refs": [],
            "privacy": "public_metadata"
        }));
        let signature = sign_object(&intent, &key, Some("alice".into())).unwrap();
        let expected_public_key = signature.public_key.clone();
        intent.signatures.push(signature);

        let batch = kfdb_index_batch("repo", "sha256:bundle", &[intent.clone()]).unwrap();

        let actor_nodes: Vec<&Value> = batch
            .operations
            .iter()
            .filter(|op| op.get("label").and_then(Value::as_str) == Some("RickydataActor"))
            .collect();
        assert_eq!(actor_nodes.len(), 1, "expected exactly one actor node");
        assert_eq!(
            actor_nodes[0]["properties"]["public_key"]["String"]
                .as_str()
                .unwrap(),
            expected_public_key
        );
        assert_eq!(
            actor_nodes[0]["properties"]["signer_label"]["String"]
                .as_str()
                .unwrap(),
            "alice"
        );

        let signed_by_edges: Vec<&Value> = batch
            .operations
            .iter()
            .filter(|op| op.get("edge_type").and_then(Value::as_str) == Some("SIGNED_BY"))
            .collect();
        assert_eq!(signed_by_edges.len(), 1, "expected one SIGNED_BY edge");
        assert_eq!(
            signed_by_edges[0]["properties"]["object_id"]["String"]
                .as_str()
                .unwrap(),
            intent.object_id
        );

        let determinism = kfdb_index_batch("repo", "sha256:bundle", &[intent.clone()]).unwrap();
        assert_eq!(batch, determinism);

        let unsigned_intent = object(serde_json::json!({
            "objective": "Unsigned intent",
            "issue_refs": [],
            "task_refs": [],
            "privacy": "public_metadata"
        }));
        let unsigned_batch = kfdb_index_batch("repo", "sha256:bundle", &[unsigned_intent]).unwrap();
        assert!(
            unsigned_batch
                .operations
                .iter()
                .all(|op| op.get("label").and_then(Value::as_str) != Some("RickydataActor")),
            "unsigned object must not emit actor nodes"
        );
        assert!(
            unsigned_batch
                .operations
                .iter()
                .all(|op| op.get("edge_type").and_then(Value::as_str) != Some("SIGNED_BY")),
            "unsigned object must not emit SIGNED_BY edges"
        );
    }

    #[test]
    fn kfdb_index_batch_emits_provenance_lineage_edges() {
        let attempt = CanonicalObject::new(
            "agent.attempt",
            DEFAULT_SCHEMA_VERSION,
            123,
            serde_json::json!({
                "attempt_id": "sha256:attempt",
                "intent_id": "sha256:intent",
            }),
        )
        .unwrap();
        let run = CanonicalObject::new(
            "agent.run",
            DEFAULT_SCHEMA_VERSION,
            123,
            serde_json::json!({
                "run_id": "sha256:run",
                "attempt_id": "sha256:attempt",
            }),
        )
        .unwrap();
        let change = CanonicalObject::new(
            "agent.change",
            DEFAULT_SCHEMA_VERSION,
            123,
            serde_json::json!({
                "change_id": "sha256:change",
                "run_ids": ["sha256:run"],
                "file_paths": ["src/lib.rs"],
            }),
        )
        .unwrap();
        let patch = CanonicalObject::new(
            "agent.patch",
            DEFAULT_SCHEMA_VERSION,
            123,
            serde_json::json!({
                "patch_id": "sha256:patch",
                "change_ids": ["sha256:change"],
                "file_paths": ["src/lib.rs"],
            }),
        )
        .unwrap();

        let batch =
            kfdb_index_batch("repo", "sha256:bundle", &[attempt, run, change, patch]).unwrap();
        let edge_types = batch
            .operations
            .iter()
            .filter_map(|op| op.get("edge_type").and_then(Value::as_str))
            .collect::<BTreeSet<_>>();

        assert!(edge_types.contains("INTENT_CHAIN"));
        assert!(edge_types.contains("ATTEMPT_LINEAGE"));
        assert!(edge_types.contains("RUN_LINEAGE"));
        assert!(edge_types.contains("PATCH_LINEAGE"));
    }

    #[test]
    fn kfdb_patch_projection_uses_patch_id_for_lineage_source() {
        let patch = CanonicalObject::new(
            "agent.patch",
            DEFAULT_SCHEMA_VERSION,
            123,
            serde_json::json!({
                "attempt_id": "sha256:attempt",
                "patch_id": "sha256:patch",
                "change_ids": ["sha256:change"],
                "file_paths": ["src/lib.rs"],
            }),
        )
        .unwrap();

        let batch = kfdb_index_batch("repo", "sha256:bundle", &[patch]).unwrap();
        let prepared_patch_node_id =
            deterministic_uuid(&["RickydataPreparedPatch", "repo", "sha256:patch"]);

        assert!(
            batch.operations.iter().any(|op| {
                op.get("operation").and_then(Value::as_str) == Some("create_node")
                    && op.get("id").and_then(Value::as_str) == Some(&prepared_patch_node_id)
                    && op.get("label").and_then(Value::as_str) == Some("RickydataPreparedPatch")
            }),
            "agent.patch projection must be keyed by patch_id, not attempt_id"
        );
        assert!(
            batch.operations.iter().any(|op| {
                op.get("edge_type").and_then(Value::as_str) == Some("PATCH_LINEAGE")
                    && op.get("from").and_then(Value::as_str) == Some(&prepared_patch_node_id)
            }),
            "PATCH_LINEAGE source must point at the created PreparedPatch node"
        );
    }

    #[test]
    fn validate_object_with_policy_rejects_tampered_signatures_under_enforcement() {
        use rickydata_git_core::{generate_signing_keypair, sign_object};

        let mut bad = object(serde_json::json!({ "issue": "local:1" }));
        let key = generate_signing_keypair();
        let mut signature = sign_object(&bad, &key, None).unwrap();
        signature.signature = "0".repeat(128);
        bad.signatures.push(signature);

        let err = validate_object_with_policy(&bad, true)
            .expect_err("tampered signature must be rejected under enforcement");
        assert!(err.to_string().contains("invalid signature"));
    }

    #[test]
    fn validate_object_with_policy_requires_signatures_when_enforced() {
        let unsigned = object(serde_json::json!({ "issue": "local:1" }));
        assert!(validate_object_with_policy(&unsigned, false).is_ok());
        let err = validate_object_with_policy(&unsigned, true)
            .expect_err("missing signature must be rejected under enforcement");
        assert!(err.to_string().contains("no signatures"));
    }

    #[test]
    fn validate_stored_object_accepts_configured_legacy_unsigned_repo() {
        let unsigned = object(serde_json::json!({ "issue": "local:1" }));
        let legacy_repos = BTreeSet::from(["legacy-repo".to_string()]);

        validate_stored_object_with_policy("legacy-repo", &unsigned, true, &legacy_repos)
            .expect("configured legacy object reads should validate canonical integrity only");

        let err = validate_stored_object_with_policy("strict-repo", &unsigned, true, &legacy_repos)
            .expect_err("unlisted repos must still require signatures under enforcement");
        assert!(err.to_string().contains("no signatures"));
    }

    #[test]
    fn validated_object_ids_accepts_configured_legacy_unsigned_repo() {
        let unsigned = object(serde_json::json!({ "issue": "local:1" }));
        let legacy_repos = BTreeSet::from(["legacy-repo".to_string()]);

        let object_ids = validated_object_ids_with_policy(
            "legacy-repo",
            std::slice::from_ref(&unsigned),
            true,
            &legacy_repos,
        )
        .expect("configured legacy repo should accept unsigned bundle ingress");
        assert_eq!(object_ids, vec![unsigned.object_id.clone()]);

        let err = validated_object_ids_with_policy("strict-repo", &[unsigned], true, &legacy_repos)
            .expect_err("unlisted repos must still require signatures under enforcement");
        assert!(err.to_string().contains("no signatures"));
    }

    #[test]
    fn validate_object_with_policy_accepts_valid_signature() {
        use rickydata_git_core::{generate_signing_keypair, sign_object};

        let mut signed = object(serde_json::json!({ "issue": "local:1" }));
        let key = generate_signing_keypair();
        let signature = sign_object(&signed, &key, None).unwrap();
        signed.signatures.push(signature);

        validate_object_with_policy(&signed, true)
            .expect("valid signature must pass under enforcement");
    }

    #[tokio::test]
    async fn http_push_pull_and_get_object_round_trip() {
        let temp = tempfile::tempdir().unwrap();
        let app = router(FileRelayStore::new(temp.path()));
        let first = object(serde_json::json!({ "issue": "local:1" }));
        let push_request = BundlePushRequest {
            repo_id: "repo".to_string(),
            idempotency_key: "http-first".to_string(),
            objects: vec![first.clone()],
        };

        let push_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/v1/repos/repo/bundles/push")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&push_request).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(push_response.status(), StatusCode::OK);
        let push: BundlePushReport = json_response(push_response).await;
        assert_eq!(push.accepted_object_count, 1);

        let pull_request = BundlePullRequest {
            repo_id: "repo".to_string(),
            known_object_ids: Vec::new(),
            limit: None,
        };
        let pull_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/v1/repos/repo/bundles/pull")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&pull_request).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(pull_response.status(), StatusCode::OK);
        let pull: BundlePullReport = json_response(pull_response).await;
        assert_eq!(pull.objects, vec![first.clone()]);

        let object_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri(format!("/v1/repos/repo/objects/{}", first.object_id))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(object_response.status(), StatusCode::OK);
        let fetched: CanonicalObject<Value> = json_response(object_response).await;
        assert_eq!(fetched, first);

        let status_response = app
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri("/v1/repos/repo/status")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(status_response.status(), StatusCode::OK);
        let status: RepoRelayStatusReport = json_response(status_response).await;
        assert_eq!(status.status, "ok");
        assert_eq!(status.object_count, 1);
    }

    #[tokio::test]
    async fn http_rejects_repo_id_mismatch() {
        let temp = tempfile::tempdir().unwrap();
        let app = router(FileRelayStore::new(temp.path()));
        let push_request = BundlePushRequest {
            repo_id: "other".to_string(),
            idempotency_key: "http-first".to_string(),
            objects: vec![object(serde_json::json!({ "issue": "local:1" }))],
        };

        let response = app
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/v1/repos/repo/bundles/push")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&push_request).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let error: RelayErrorReport = json_response(response).await;
        assert!(error.message.contains("does not match path repo_id"));
    }

    fn status_request(token: Option<&str>) -> Request<Body> {
        let mut builder = Request::builder()
            .method(Method::GET)
            .uri("/v1/repos/repo/status");
        if let Some(token) = token {
            builder = builder.header("authorization", format!("Bearer {token}"));
        }
        builder.body(Body::empty()).unwrap()
    }

    #[tokio::test]
    async fn auth_disabled_when_no_token_configured() {
        let temp = tempfile::tempdir().unwrap();
        let app = router_with_auth(FileRelayStore::new(temp.path()), None);
        let response = app.oneshot(status_request(None)).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn auth_accepts_correct_bearer_token() {
        let temp = tempfile::tempdir().unwrap();
        let app = router_with_auth(FileRelayStore::new(temp.path()), Some("s3cr3t".to_string()));
        let response = app.oneshot(status_request(Some("s3cr3t"))).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn auth_rejects_missing_or_wrong_bearer_token() {
        let temp = tempfile::tempdir().unwrap();
        let app = router_with_auth(FileRelayStore::new(temp.path()), Some("s3cr3t".to_string()));
        let missing = app.clone().oneshot(status_request(None)).await.unwrap();
        assert_eq!(missing.status(), StatusCode::UNAUTHORIZED);
        let wrong = app.oneshot(status_request(Some("nope"))).await.unwrap();
        assert_eq!(wrong.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn auth_always_allows_health_unauthenticated() {
        let temp = tempfile::tempdir().unwrap();
        let app = router_with_auth(FileRelayStore::new(temp.path()), Some("s3cr3t".to_string()));
        let response = app
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri("/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let report: HealthReport = json_response(response).await;
        assert_eq!(report.status, "ok");
    }
}
