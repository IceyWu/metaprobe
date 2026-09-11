use sha2::{Digest, Sha256};

/// Compute SHA-256 hex digest of the given bytes.
pub fn sha256_hex(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hex::encode(hasher.finalize())
}
