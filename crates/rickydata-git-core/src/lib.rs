use std::fs;
use std::io::Write as _;
use std::path::Path;

use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Key, Nonce};
use ed25519_dalek::{
    PUBLIC_KEY_LENGTH, SECRET_KEY_LENGTH, SIGNATURE_LENGTH, Signature, Signer, SigningKey,
    Verifier, VerifyingKey,
};
use rand::RngCore;
use rand::rngs::OsRng;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Number, Value};
use sha2::{Digest, Sha256};

pub const DEFAULT_SCHEMA_VERSION: &str = "rickydata.git.v1";
pub const SIGNATURE_ALGORITHM_ED25519: &str = "ed25519";
pub const ENCRYPTION_ALGORITHM_AES_256_GCM: &str = "aes-256-gcm";
pub const AES_256_KEY_LENGTH: usize = 32;
pub const AES_GCM_NONCE_LENGTH: usize = 12;

#[derive(Debug, thiserror::Error)]
pub enum CoreError {
    #[error("failed to serialize value for canonical hashing: {0}")]
    Serialize(#[from] serde_json::Error),
    #[error("signing error: {0}")]
    Signing(String),
    #[error("encryption error: {0}")]
    Encryption(String),
    #[error("tee policy error: {0}")]
    Tee(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncryptedBody {
    pub envelope: EncryptionEnvelopeRef,
    pub ciphertext: Vec<u8>,
    pub nonce: [u8; AES_GCM_NONCE_LENGTH],
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PrivacyClass {
    PublicMetadata,
    PrivateBody,
    Secret,
    Encrypted,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct EncryptionEnvelopeRef {
    pub algorithm: String,
    pub envelope_hash: String,
    pub key_ref: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SignerReceiptRef {
    pub receipt_hash: String,
    pub operation: String,
    pub trust_bundle_hash: String,
    pub counter_namespace: String,
    pub counter_value: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct TeePolicy {
    pub required: bool,
    pub accepted_trust_bundle_hashes: Vec<String>,
    pub accepted_operations: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ReleaseGuardPolicy {
    pub required: bool,
    pub reason: String,
    pub signer_receipt: Option<SignerReceiptRef>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ActorSignature {
    pub algorithm: String,
    pub public_key: String,
    pub signature: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signed_at_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signer_label: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SignedRefExpectation {
    pub ref_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_previous_oid: Option<String>,
    pub new_oid: String,
    pub signature: ActorSignature,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SourceSpan {
    pub file_path: String,
    pub start_line: u32,
    pub start_column: u32,
    pub end_line: u32,
    pub end_column: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SymbolRef {
    pub language: String,
    pub file_path: String,
    pub symbol_name: String,
    pub range: Option<SourceSpan>,
    pub content_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct CanonicalObject<T> {
    pub schema_version: String,
    pub kind: String,
    pub object_id: String,
    pub created_at_ms: u64,
    pub body: T,
    pub body_hash: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub signatures: Vec<ActorSignature>,
}

impl<T> CanonicalObject<T>
where
    T: Serialize,
{
    pub fn new(
        kind: impl Into<String>,
        schema_version: impl Into<String>,
        created_at_ms: u64,
        body: T,
    ) -> Result<Self, CoreError> {
        let kind = kind.into();
        let schema_version = schema_version.into();
        let body_value = serde_json::to_value(&body)?;
        let body_hash = stable_json_hash(&body_value)?;
        let object_id = canonical_object_id(&kind, &schema_version, &body_value)?;

        Ok(Self {
            schema_version,
            kind,
            object_id,
            created_at_ms,
            body,
            body_hash,
            signatures: Vec::new(),
        })
    }
}

pub fn canonical_object_id(
    kind: &str,
    schema_version: &str,
    body: &Value,
) -> Result<String, CoreError> {
    stable_json_hash(&serde_json::json!({
        "kind": kind,
        "schema_version": schema_version,
        "body": body,
    }))
}

pub fn signing_message(
    kind: &str,
    schema_version: &str,
    body: &Value,
) -> Result<Vec<u8>, CoreError> {
    let canonical = canonical_json(&serde_json::json!({
        "body": body,
        "kind": kind,
        "schema_version": schema_version,
    }));
    Ok(serde_json::to_vec(&canonical)?)
}

pub fn sign_object<T: Serialize>(
    object: &CanonicalObject<T>,
    signing_key: &SigningKey,
    signer_label: Option<String>,
) -> Result<ActorSignature, CoreError> {
    let body_value = serde_json::to_value(&object.body)?;
    let message = signing_message(&object.kind, &object.schema_version, &body_value)?;
    let signature = signing_key.sign(&message);
    let verifying_key = signing_key.verifying_key();

    Ok(ActorSignature {
        algorithm: SIGNATURE_ALGORITHM_ED25519.to_string(),
        public_key: hex::encode(verifying_key.to_bytes()),
        signature: hex::encode(signature.to_bytes()),
        signed_at_ms: None,
        signer_label,
    })
}

pub fn verify_signature(
    kind: &str,
    schema_version: &str,
    body: &Value,
    signature: &ActorSignature,
) -> Result<bool, CoreError> {
    if signature.algorithm != SIGNATURE_ALGORITHM_ED25519 {
        return Err(CoreError::Signing(format!(
            "unsupported signature algorithm: {}",
            signature.algorithm
        )));
    }

    let public_key_bytes = hex::decode(&signature.public_key)
        .map_err(|err| CoreError::Signing(format!("public_key hex decode failed: {err}")))?;
    let public_key_array: [u8; PUBLIC_KEY_LENGTH] = public_key_bytes
        .as_slice()
        .try_into()
        .map_err(|_| CoreError::Signing(format!("public_key must be {PUBLIC_KEY_LENGTH} bytes")))?;
    let verifying_key = VerifyingKey::from_bytes(&public_key_array)
        .map_err(|err| CoreError::Signing(format!("invalid verifying key: {err}")))?;

    let signature_bytes = hex::decode(&signature.signature)
        .map_err(|err| CoreError::Signing(format!("signature hex decode failed: {err}")))?;
    let signature_array: [u8; SIGNATURE_LENGTH] = signature_bytes
        .as_slice()
        .try_into()
        .map_err(|_| CoreError::Signing(format!("signature must be {SIGNATURE_LENGTH} bytes")))?;
    let parsed_signature = Signature::from_bytes(&signature_array);

    let message = signing_message(kind, schema_version, body)?;
    Ok(verifying_key.verify(&message, &parsed_signature).is_ok())
}

pub fn signed_ref_expectation_message(
    ref_name: &str,
    expected_previous_oid: Option<&str>,
    new_oid: &str,
) -> Result<Vec<u8>, CoreError> {
    let canonical = canonical_json(&serde_json::json!({
        "expected_previous_oid": expected_previous_oid,
        "new_oid": new_oid,
        "ref_name": ref_name,
    }));
    Ok(serde_json::to_vec(&canonical)?)
}

pub fn sign_ref_expectation(
    ref_name: &str,
    expected_previous_oid: Option<&str>,
    new_oid: &str,
    signing_key: &SigningKey,
    signer_label: Option<String>,
) -> Result<SignedRefExpectation, CoreError> {
    let message = signed_ref_expectation_message(ref_name, expected_previous_oid, new_oid)?;
    let sig = signing_key.sign(&message);
    let verifying_key = signing_key.verifying_key();
    Ok(SignedRefExpectation {
        ref_name: ref_name.to_string(),
        expected_previous_oid: expected_previous_oid.map(str::to_string),
        new_oid: new_oid.to_string(),
        signature: ActorSignature {
            algorithm: SIGNATURE_ALGORITHM_ED25519.to_string(),
            public_key: hex::encode(verifying_key.to_bytes()),
            signature: hex::encode(sig.to_bytes()),
            signed_at_ms: None,
            signer_label,
        },
    })
}

pub fn verify_ref_expectation_signature(
    expectation: &SignedRefExpectation,
) -> Result<bool, CoreError> {
    let signature = &expectation.signature;
    if signature.algorithm != SIGNATURE_ALGORITHM_ED25519 {
        return Err(CoreError::Signing(format!(
            "unsupported signature algorithm: {}",
            signature.algorithm
        )));
    }

    let public_key_bytes = hex::decode(&signature.public_key)
        .map_err(|err| CoreError::Signing(format!("public_key hex decode failed: {err}")))?;
    let public_key_array: [u8; PUBLIC_KEY_LENGTH] = public_key_bytes
        .as_slice()
        .try_into()
        .map_err(|_| CoreError::Signing(format!("public_key must be {PUBLIC_KEY_LENGTH} bytes")))?;
    let verifying_key = VerifyingKey::from_bytes(&public_key_array)
        .map_err(|err| CoreError::Signing(format!("invalid verifying key: {err}")))?;

    let signature_bytes = hex::decode(&signature.signature)
        .map_err(|err| CoreError::Signing(format!("signature hex decode failed: {err}")))?;
    let signature_array: [u8; SIGNATURE_LENGTH] = signature_bytes
        .as_slice()
        .try_into()
        .map_err(|_| CoreError::Signing(format!("signature must be {SIGNATURE_LENGTH} bytes")))?;
    let parsed_signature = Signature::from_bytes(&signature_array);

    let message = signed_ref_expectation_message(
        &expectation.ref_name,
        expectation.expected_previous_oid.as_deref(),
        &expectation.new_oid,
    )?;
    Ok(verifying_key.verify(&message, &parsed_signature).is_ok())
}

pub fn generate_signing_keypair() -> SigningKey {
    SigningKey::generate(&mut OsRng)
}

pub fn load_signing_key_from_file(path: &Path) -> Result<SigningKey, CoreError> {
    let raw = fs::read(path)
        .map_err(|err| CoreError::Signing(format!("failed to read signing key: {err}")))?;

    let seed = if raw.len() == SECRET_KEY_LENGTH {
        let mut buf = [0u8; SECRET_KEY_LENGTH];
        buf.copy_from_slice(&raw);
        buf
    } else {
        let text = std::str::from_utf8(&raw)
            .map_err(|err| {
                CoreError::Signing(format!("signing key is not raw bytes or utf-8: {err}"))
            })?
            .trim();
        let decoded = hex::decode(text)
            .map_err(|err| CoreError::Signing(format!("signing key hex decode failed: {err}")))?;
        if decoded.len() != SECRET_KEY_LENGTH {
            return Err(CoreError::Signing(format!(
                "decoded signing key must be {SECRET_KEY_LENGTH} bytes, got {}",
                decoded.len()
            )));
        }
        let mut buf = [0u8; SECRET_KEY_LENGTH];
        buf.copy_from_slice(&decoded);
        buf
    };

    Ok(SigningKey::from_bytes(&seed))
}

pub fn save_signing_key_to_file(key: &SigningKey, path: &Path) -> Result<(), CoreError> {
    let bytes = key.to_bytes();
    let mut file = fs::File::create(path)
        .map_err(|err| CoreError::Signing(format!("failed to create signing key file: {err}")))?;
    file.write_all(&bytes)
        .map_err(|err| CoreError::Signing(format!("failed to write signing key: {err}")))?;
    // A signing key is a long-lived secret: restrict it to owner read/write only.
    // This is the single chokepoint for both `key generate` and `key init`.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o600)).map_err(|err| {
            CoreError::Signing(format!("failed to chmod 600 signing key file: {err}"))
        })?;
    }
    Ok(())
}

pub fn generate_encryption_key() -> [u8; AES_256_KEY_LENGTH] {
    let mut bytes = [0u8; AES_256_KEY_LENGTH];
    OsRng.fill_bytes(&mut bytes);
    bytes
}

pub fn load_encryption_key_from_file(path: &Path) -> Result<[u8; AES_256_KEY_LENGTH], CoreError> {
    let raw = fs::read(path)
        .map_err(|err| CoreError::Encryption(format!("failed to read encryption key: {err}")))?;

    let bytes = if raw.len() == AES_256_KEY_LENGTH {
        raw
    } else {
        let text = std::str::from_utf8(&raw)
            .map_err(|err| {
                CoreError::Encryption(format!("encryption key is not raw bytes or utf-8: {err}"))
            })?
            .trim();
        hex::decode(text).map_err(|err| {
            CoreError::Encryption(format!("encryption key hex decode failed: {err}"))
        })?
    };

    if bytes.len() != AES_256_KEY_LENGTH {
        return Err(CoreError::Encryption(format!(
            "encryption key must be {AES_256_KEY_LENGTH} bytes, got {}",
            bytes.len()
        )));
    }
    let mut out = [0u8; AES_256_KEY_LENGTH];
    out.copy_from_slice(&bytes);
    Ok(out)
}

pub fn encrypt_body(
    plaintext: &[u8],
    key: &[u8; AES_256_KEY_LENGTH],
    key_ref: Option<String>,
) -> Result<EncryptedBody, CoreError> {
    let cipher_key = Key::<Aes256Gcm>::from_slice(key);
    let cipher = Aes256Gcm::new(cipher_key);

    let mut nonce_bytes = [0u8; AES_GCM_NONCE_LENGTH];
    OsRng.fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);

    let ciphertext = cipher
        .encrypt(nonce, plaintext)
        .map_err(|err| CoreError::Encryption(format!("aes-256-gcm encrypt failed: {err}")))?;

    let mut hasher = Sha256::new();
    hasher.update(nonce_bytes);
    hasher.update(&ciphertext);
    let envelope_hash = format!("sha256:{}", hex::encode(hasher.finalize()));

    Ok(EncryptedBody {
        envelope: EncryptionEnvelopeRef {
            algorithm: ENCRYPTION_ALGORITHM_AES_256_GCM.to_string(),
            envelope_hash,
            key_ref,
        },
        ciphertext,
        nonce: nonce_bytes,
    })
}

pub fn decrypt_body(
    envelope: &EncryptionEnvelopeRef,
    ciphertext: &[u8],
    nonce: &[u8; AES_GCM_NONCE_LENGTH],
    key: &[u8; AES_256_KEY_LENGTH],
) -> Result<Vec<u8>, CoreError> {
    if envelope.algorithm != ENCRYPTION_ALGORITHM_AES_256_GCM {
        return Err(CoreError::Encryption(format!(
            "unsupported encryption algorithm: {}",
            envelope.algorithm
        )));
    }

    let mut hasher = Sha256::new();
    hasher.update(nonce);
    hasher.update(ciphertext);
    let envelope_hash = format!("sha256:{}", hex::encode(hasher.finalize()));
    if envelope_hash != envelope.envelope_hash {
        return Err(CoreError::Encryption(
            "envelope_hash does not match ciphertext+nonce".to_string(),
        ));
    }

    let cipher_key = Key::<Aes256Gcm>::from_slice(key);
    let cipher = Aes256Gcm::new(cipher_key);
    let nonce = Nonce::from_slice(nonce);
    cipher
        .decrypt(nonce, ciphertext)
        .map_err(|err| CoreError::Encryption(format!("aes-256-gcm decrypt failed: {err}")))
}

pub fn verify_signer_receipt_stub(receipt: &SignerReceiptRef) -> Result<bool, CoreError> {
    if receipt.receipt_hash.trim().is_empty() {
        return Err(CoreError::Tee("receipt_hash must not be empty".into()));
    }
    if !receipt.receipt_hash.starts_with("sha256:") {
        return Err(CoreError::Tee(
            "receipt_hash must be a sha256: prefixed digest".into(),
        ));
    }
    if receipt.operation.trim().is_empty() {
        return Err(CoreError::Tee("operation must not be empty".into()));
    }
    if receipt.trust_bundle_hash.trim().is_empty() {
        return Err(CoreError::Tee("trust_bundle_hash must not be empty".into()));
    }
    if !receipt.trust_bundle_hash.starts_with("sha256:") {
        return Err(CoreError::Tee(
            "trust_bundle_hash must be a sha256: prefixed digest".into(),
        ));
    }
    if receipt.counter_namespace.trim().is_empty() {
        return Err(CoreError::Tee("counter_namespace must not be empty".into()));
    }
    if receipt.counter_value == 0 {
        return Err(CoreError::Tee(
            "counter_value must be greater than 0".into(),
        ));
    }
    Ok(true)
}

pub fn verify_tee_policy_stub(
    policy: &TeePolicy,
    receipt: &SignerReceiptRef,
) -> Result<bool, CoreError> {
    verify_signer_receipt_stub(receipt)?;

    if !policy
        .accepted_trust_bundle_hashes
        .iter()
        .any(|hash| hash == &receipt.trust_bundle_hash)
    {
        return Err(CoreError::Tee(format!(
            "trust_bundle_hash {} not in accepted_trust_bundle_hashes",
            receipt.trust_bundle_hash
        )));
    }

    if !policy
        .accepted_operations
        .iter()
        .any(|op| op == &receipt.operation)
    {
        return Err(CoreError::Tee(format!(
            "operation {} not in accepted_operations",
            receipt.operation
        )));
    }

    Ok(true)
}

pub fn stable_json_hash(value: &Value) -> Result<String, CoreError> {
    let canonical = canonical_json(value);
    let bytes = serde_json::to_vec(&canonical)?;
    let digest = Sha256::digest(bytes);
    Ok(format!("sha256:{}", hex::encode(digest)))
}

pub fn canonical_json(value: &Value) -> Value {
    match value {
        Value::Array(values) => Value::Array(values.iter().map(canonical_json).collect()),
        Value::Number(number) => canonical_number(number),
        Value::Object(object) => {
            let mut entries = object.iter().collect::<Vec<_>>();
            entries.sort_by_key(|(key, _)| *key);

            let mut sorted = Map::new();
            for (key, value) in entries {
                sorted.insert(key.clone(), canonical_json(value));
            }
            Value::Object(sorted)
        }
        other => other.clone(),
    }
}

fn canonical_number(number: &Number) -> Value {
    if let Some(value) = number.as_i64() {
        return Value::Number(Number::from(value));
    }
    if let Some(value) = number.as_u64() {
        return Value::Number(Number::from(value));
    }
    if let Some(value) = number.as_f64() {
        if value.is_finite()
            && value.fract() == 0.0
            && value >= i64::MIN as f64
            && value <= i64::MAX as f64
        {
            return Value::Number(Number::from(value as i64));
        }
        if let Some(number) = Number::from_f64(value) {
            return Value::Number(number);
        }
    }
    Value::Number(number.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, serde::Deserialize)]
    struct HashVectorCatalog {
        vectors: Vec<HashVector>,
    }

    #[derive(Debug, serde::Deserialize)]
    struct HashVector {
        name: String,
        kind: String,
        schema_version: String,
        body: Value,
        canonical_body_json: String,
        body_hash: String,
        object_id: String,
    }

    #[test]
    fn canonical_hash_is_independent_of_object_key_order() {
        let left = serde_json::json!({
            "b": 2,
            "a": { "z": true, "m": [3, 2, 1] }
        });
        let right = serde_json::json!({
            "a": { "m": [3, 2, 1], "z": true },
            "b": 2
        });

        assert_eq!(
            stable_json_hash(&left).unwrap(),
            stable_json_hash(&right).unwrap()
        );
    }

    #[test]
    fn object_id_ignores_transport_metadata() {
        let first = CanonicalObject::new(
            "agent.intent",
            DEFAULT_SCHEMA_VERSION,
            100,
            serde_json::json!({ "objective": "fix issue", "issue": 42 }),
        )
        .unwrap();
        let second = CanonicalObject::new(
            "agent.intent",
            DEFAULT_SCHEMA_VERSION,
            200,
            serde_json::json!({ "issue": 42, "objective": "fix issue" }),
        )
        .unwrap();

        assert_eq!(first.object_id, second.object_id);
        assert_eq!(first.body_hash, second.body_hash);
        assert_ne!(first.created_at_ms, second.created_at_ms);
    }

    #[test]
    fn unsigned_objects_round_trip_with_empty_signatures() {
        let object = CanonicalObject::new(
            "agent.intent",
            DEFAULT_SCHEMA_VERSION,
            500,
            serde_json::json!({ "objective": "round trip" }),
        )
        .unwrap();
        let serialized = serde_json::to_string(&object).unwrap();
        assert!(
            !serialized.contains("\"signatures\""),
            "empty signatures must not appear on the wire: {serialized}"
        );
        let parsed: CanonicalObject<Value> = serde_json::from_str(&serialized).unwrap();
        assert!(parsed.signatures.is_empty());
        assert_eq!(parsed.object_id, object.object_id);
        assert_eq!(parsed.body_hash, object.body_hash);
    }

    #[test]
    fn signed_object_preserves_object_id() {
        let mut object = CanonicalObject::new(
            "agent.intent",
            DEFAULT_SCHEMA_VERSION,
            600,
            serde_json::json!({ "objective": "stable id" }),
        )
        .unwrap();
        let original_id = object.object_id.clone();
        let original_body_hash = object.body_hash.clone();

        let key = generate_signing_keypair();
        let signature = sign_object(&object, &key, Some("alice".into())).unwrap();
        object.signatures.push(signature);

        assert_eq!(object.object_id, original_id);
        assert_eq!(object.body_hash, original_body_hash);

        let recomputed = canonical_object_id(
            &object.kind,
            &object.schema_version,
            &serde_json::to_value(&object.body).unwrap(),
        )
        .unwrap();
        assert_eq!(recomputed, original_id);
    }

    #[test]
    fn signature_verifies_against_canonical_content() {
        let object = CanonicalObject::new(
            "agent.intent",
            DEFAULT_SCHEMA_VERSION,
            700,
            serde_json::json!({ "objective": "verify me", "issue": 7 }),
        )
        .unwrap();
        let key = generate_signing_keypair();
        let signature = sign_object(&object, &key, None).unwrap();

        let body_value = serde_json::to_value(&object.body).unwrap();
        let ok = verify_signature(
            &object.kind,
            &object.schema_version,
            &body_value,
            &signature,
        )
        .unwrap();
        assert!(ok, "signature should verify against canonical content");
    }

    #[test]
    fn tampered_body_fails_verification() {
        let object = CanonicalObject::new(
            "agent.intent",
            DEFAULT_SCHEMA_VERSION,
            800,
            serde_json::json!({ "objective": "do thing", "issue": 1 }),
        )
        .unwrap();
        let key = generate_signing_keypair();
        let signature = sign_object(&object, &key, None).unwrap();

        let tampered = serde_json::json!({ "objective": "do other thing", "issue": 1 });
        let ok =
            verify_signature(&object.kind, &object.schema_version, &tampered, &signature).unwrap();
        assert!(!ok, "tampered body must fail signature verification");
    }

    #[test]
    fn multiple_signatures_supported() {
        let mut object = CanonicalObject::new(
            "agent.intent",
            DEFAULT_SCHEMA_VERSION,
            900,
            serde_json::json!({ "objective": "co-sign" }),
        )
        .unwrap();
        let key_a = generate_signing_keypair();
        let key_b = generate_signing_keypair();
        let sig_a = sign_object(&object, &key_a, Some("alice".into())).unwrap();
        let sig_b = sign_object(&object, &key_b, Some("bob".into())).unwrap();
        object.signatures.push(sig_a);
        object.signatures.push(sig_b);

        let body_value = serde_json::to_value(&object.body).unwrap();
        for signature in &object.signatures {
            let ok = verify_signature(&object.kind, &object.schema_version, &body_value, signature)
                .unwrap();
            assert!(ok, "each signature must verify independently");
        }
        assert_ne!(
            object.signatures[0].public_key,
            object.signatures[1].public_key
        );
    }

    #[test]
    fn signing_key_round_trips_through_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("key.bin");
        let key = generate_signing_keypair();
        save_signing_key_to_file(&key, &path).unwrap();
        let loaded = load_signing_key_from_file(&path).unwrap();
        assert_eq!(key.to_bytes(), loaded.to_bytes());
    }

    #[cfg(unix)]
    #[test]
    fn signing_key_file_is_owner_only() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("key.bin");
        let key = generate_signing_keypair();
        save_signing_key_to_file(&key, &path).unwrap();
        let mode = std::fs::metadata(&path).unwrap().permissions().mode();
        assert_eq!(
            mode & 0o777,
            0o600,
            "signing key file must be -rw------- (0o600), got {:o}",
            mode & 0o777
        );
    }

    #[test]
    fn signing_key_accepts_hex_format() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("key.hex");
        let key = generate_signing_keypair();
        let hex = hex::encode(key.to_bytes());
        std::fs::write(&path, format!("{hex}\n")).unwrap();
        let loaded = load_signing_key_from_file(&path).unwrap();
        assert_eq!(key.to_bytes(), loaded.to_bytes());
    }

    #[test]
    fn encrypt_decrypt_round_trip_recovers_plaintext() {
        let key = generate_encryption_key();
        let plaintext = b"agent stdout / stderr contents that should be private";
        let encrypted = encrypt_body(plaintext, &key, Some("local:dev".into())).unwrap();
        assert_eq!(
            encrypted.envelope.algorithm,
            ENCRYPTION_ALGORITHM_AES_256_GCM
        );
        assert!(encrypted.envelope.envelope_hash.starts_with("sha256:"));
        assert_eq!(encrypted.envelope.key_ref.as_deref(), Some("local:dev"));
        assert_ne!(&encrypted.ciphertext[..], plaintext);

        let decrypted = decrypt_body(
            &encrypted.envelope,
            &encrypted.ciphertext,
            &encrypted.nonce,
            &key,
        )
        .unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn decrypt_fails_when_envelope_hash_drifts() {
        let key = generate_encryption_key();
        let mut encrypted = encrypt_body(b"hello", &key, None).unwrap();
        encrypted.envelope.envelope_hash =
            "sha256:0000000000000000000000000000000000000000000000000000000000000000".to_string();
        let err = decrypt_body(
            &encrypted.envelope,
            &encrypted.ciphertext,
            &encrypted.nonce,
            &key,
        )
        .unwrap_err();
        match err {
            CoreError::Encryption(msg) => assert!(msg.contains("envelope_hash")),
            other => panic!("expected Encryption error, got {other:?}"),
        }
    }

    #[test]
    fn decrypt_fails_with_wrong_key() {
        let key_a = generate_encryption_key();
        let key_b = generate_encryption_key();
        let encrypted = encrypt_body(b"secret", &key_a, None).unwrap();
        let err = decrypt_body(
            &encrypted.envelope,
            &encrypted.ciphertext,
            &encrypted.nonce,
            &key_b,
        )
        .unwrap_err();
        match err {
            CoreError::Encryption(_) => {}
            other => panic!("expected Encryption error, got {other:?}"),
        }
    }

    #[test]
    fn encryption_key_file_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let raw_path = dir.path().join("k.bin");
        let key = generate_encryption_key();
        std::fs::write(&raw_path, key).unwrap();
        let loaded = load_encryption_key_from_file(&raw_path).unwrap();
        assert_eq!(loaded, key);

        let hex_path = dir.path().join("k.hex");
        std::fs::write(&hex_path, format!("{}\n", hex::encode(key))).unwrap();
        let loaded_hex = load_encryption_key_from_file(&hex_path).unwrap();
        assert_eq!(loaded_hex, key);
    }

    fn well_formed_receipt() -> SignerReceiptRef {
        SignerReceiptRef {
            receipt_hash: "sha256:aaaa".into(),
            operation: "sign_release".into(),
            trust_bundle_hash: "sha256:bbbb".into(),
            counter_namespace: "ns:release".into(),
            counter_value: 42,
        }
    }

    #[test]
    fn signer_receipt_stub_accepts_well_formed_receipt() {
        let receipt = well_formed_receipt();
        assert!(verify_signer_receipt_stub(&receipt).unwrap());
    }

    #[test]
    fn signer_receipt_stub_rejects_missing_fields() {
        let mut receipt = well_formed_receipt();
        receipt.receipt_hash = String::new();
        let err = verify_signer_receipt_stub(&receipt).unwrap_err();
        assert!(matches!(err, CoreError::Tee(_)));

        let mut receipt = well_formed_receipt();
        receipt.operation = "   ".into();
        assert!(matches!(
            verify_signer_receipt_stub(&receipt).unwrap_err(),
            CoreError::Tee(_)
        ));

        let mut receipt = well_formed_receipt();
        receipt.counter_value = 0;
        assert!(matches!(
            verify_signer_receipt_stub(&receipt).unwrap_err(),
            CoreError::Tee(_)
        ));
    }

    #[test]
    fn signer_receipt_stub_rejects_unprefixed_hashes() {
        let mut receipt = well_formed_receipt();
        receipt.receipt_hash = "deadbeef".into();
        let err = verify_signer_receipt_stub(&receipt).unwrap_err();
        match err {
            CoreError::Tee(msg) => assert!(msg.contains("sha256:")),
            other => panic!("expected Tee error, got {other:?}"),
        }
    }

    #[test]
    fn tee_policy_stub_accepts_matching_receipt() {
        let receipt = well_formed_receipt();
        let policy = TeePolicy {
            required: true,
            accepted_trust_bundle_hashes: vec![receipt.trust_bundle_hash.clone()],
            accepted_operations: vec![receipt.operation.clone()],
        };
        assert!(verify_tee_policy_stub(&policy, &receipt).unwrap());
    }

    #[test]
    fn tee_policy_stub_rejects_unknown_trust_bundle() {
        let receipt = well_formed_receipt();
        let policy = TeePolicy {
            required: true,
            accepted_trust_bundle_hashes: vec!["sha256:other".into()],
            accepted_operations: vec![receipt.operation.clone()],
        };
        let err = verify_tee_policy_stub(&policy, &receipt).unwrap_err();
        match err {
            CoreError::Tee(msg) => assert!(msg.contains("trust_bundle_hash")),
            other => panic!("expected Tee error, got {other:?}"),
        }
    }

    #[test]
    fn tee_policy_stub_rejects_mismatched_operation() {
        let receipt = well_formed_receipt();
        let policy = TeePolicy {
            required: true,
            accepted_trust_bundle_hashes: vec![receipt.trust_bundle_hash.clone()],
            accepted_operations: vec!["other_op".into()],
        };
        let err = verify_tee_policy_stub(&policy, &receipt).unwrap_err();
        match err {
            CoreError::Tee(msg) => assert!(msg.contains("operation")),
            other => panic!("expected Tee error, got {other:?}"),
        }
    }

    #[test]
    fn tee_policy_stub_propagates_malformed_receipt() {
        let mut receipt = well_formed_receipt();
        receipt.counter_value = 0;
        let policy = TeePolicy {
            required: true,
            accepted_trust_bundle_hashes: vec![receipt.trust_bundle_hash.clone()],
            accepted_operations: vec![receipt.operation.clone()],
        };
        assert!(matches!(
            verify_tee_policy_stub(&policy, &receipt).unwrap_err(),
            CoreError::Tee(_)
        ));
    }

    #[test]
    fn signed_ref_expectation_round_trips_and_verifies() {
        let key = generate_signing_keypair();
        let expectation = sign_ref_expectation(
            "refs/rickydata/intents/abc",
            Some("deadbeef"),
            "cafef00d",
            &key,
            Some("alice".into()),
        )
        .unwrap();
        assert_eq!(expectation.ref_name, "refs/rickydata/intents/abc");
        assert_eq!(
            expectation.expected_previous_oid.as_deref(),
            Some("deadbeef")
        );
        assert_eq!(expectation.new_oid, "cafef00d");
        assert!(verify_ref_expectation_signature(&expectation).unwrap());
    }

    #[test]
    fn signed_ref_expectation_supports_create_only() {
        let key = generate_signing_keypair();
        let expectation =
            sign_ref_expectation("refs/rickydata/intents/new", None, "1111", &key, None).unwrap();
        assert!(expectation.expected_previous_oid.is_none());
        assert!(verify_ref_expectation_signature(&expectation).unwrap());

        let serialized = serde_json::to_string(&expectation).unwrap();
        assert!(
            !serialized.contains("expected_previous_oid"),
            "None expected_previous_oid must be omitted on the wire: {serialized}"
        );
    }

    #[test]
    fn signed_ref_expectation_rejects_tampered_new_oid() {
        let key = generate_signing_keypair();
        let mut expectation =
            sign_ref_expectation("refs/r/a", Some("aaaa"), "bbbb", &key, None).unwrap();
        expectation.new_oid = "cccc".into();
        assert!(!verify_ref_expectation_signature(&expectation).unwrap());
    }

    #[test]
    fn signed_ref_expectation_rejects_tampered_ref_name() {
        let key = generate_signing_keypair();
        let mut expectation =
            sign_ref_expectation("refs/r/a", Some("aaaa"), "bbbb", &key, None).unwrap();
        expectation.ref_name = "refs/r/b".into();
        assert!(!verify_ref_expectation_signature(&expectation).unwrap());
    }

    #[test]
    fn signed_ref_expectation_rejects_tampered_previous_oid() {
        let key = generate_signing_keypair();
        let mut expectation =
            sign_ref_expectation("refs/r/a", Some("aaaa"), "bbbb", &key, None).unwrap();
        expectation.expected_previous_oid = Some("dddd".into());
        assert!(!verify_ref_expectation_signature(&expectation).unwrap());
    }

    #[test]
    fn signed_ref_expectation_message_is_canonical() {
        let m1 = signed_ref_expectation_message("refs/r/a", Some("aaaa"), "bbbb").unwrap();
        let m2 = signed_ref_expectation_message("refs/r/a", Some("aaaa"), "bbbb").unwrap();
        assert_eq!(m1, m2);
        let differs_on_new =
            signed_ref_expectation_message("refs/r/a", Some("aaaa"), "cccc").unwrap();
        assert_ne!(m1, differs_on_new);
        let create_only = signed_ref_expectation_message("refs/r/a", None, "bbbb").unwrap();
        assert_ne!(m1, create_only);
    }

    #[test]
    fn canonical_hash_matches_golden_vectors() {
        let catalog: HashVectorCatalog = serde_json::from_str(include_str!(
            "../../../fixtures/canonical-hash-vectors.json"
        ))
        .unwrap();

        for vector in catalog.vectors {
            let canonical_body = serde_json::to_string(&canonical_json(&vector.body)).unwrap();
            assert_eq!(
                canonical_body, vector.canonical_body_json,
                "canonical JSON drifted for {}",
                vector.name
            );
            assert_eq!(
                stable_json_hash(&vector.body).unwrap(),
                vector.body_hash,
                "body hash drifted for {}",
                vector.name
            );
            assert_eq!(
                canonical_object_id(&vector.kind, &vector.schema_version, &vector.body).unwrap(),
                vector.object_id,
                "object id drifted for {}",
                vector.name
            );
        }
    }
}
