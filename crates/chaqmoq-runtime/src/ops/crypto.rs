//! `crypto` globals: CSPRNG access, UUIDs and digests.

use deno_core::op2;
use deno_error::JsErrorBox;
use rand::TryRngCore;
use sha1::Digest as _;
use sha1::Sha1;
use sha2::Sha256;
use sha2::Sha384;
use sha2::Sha512;

/// Fills the buffer from the OS CSPRNG, exactly like `crypto.getRandomValues`.
#[op2(fast)]
pub fn op_chq_get_random_values(#[buffer] out: &mut [u8]) -> Result<(), JsErrorBox> {
    // The Web Crypto spec caps a single call at 64 KiB.
    if out.len() > 65_536 {
        return Err(JsErrorBox::new(
            "QuotaExceededError",
            "The ArrayBufferView's byte length exceeds 65536",
        ));
    }
    rand::rngs::OsRng
        .try_fill_bytes(out)
        .map_err(|e| JsErrorBox::generic(format!("failed to read from the system CSPRNG: {e}")))
}

#[op2]
#[string]
pub fn op_chq_random_uuid() -> String {
    uuid::Uuid::new_v4().to_string()
}

/// Backs `crypto.subtle.digest`. `algorithm` is normalised on the JS side.
#[op2]
#[buffer]
pub fn op_chq_digest(
    #[string] algorithm: String,
    #[buffer] data: &[u8],
) -> Result<Vec<u8>, JsErrorBox> {
    let digest = match algorithm.as_str() {
        "SHA-1" => Sha1::digest(data).to_vec(),
        "SHA-256" => Sha256::digest(data).to_vec(),
        "SHA-384" => Sha384::digest(data).to_vec(),
        "SHA-512" => Sha512::digest(data).to_vec(),
        other => {
            return Err(JsErrorBox::new(
                "NotSupportedError",
                format!("unsupported digest algorithm: {other}"),
            ))
        }
    };
    Ok(digest)
}
