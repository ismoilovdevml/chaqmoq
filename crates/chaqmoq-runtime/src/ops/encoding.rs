//! `btoa` / `atob` — base64 over Latin-1, per the HTML spec.

use base64::engine::general_purpose::STANDARD;
use base64::Engine as _;
use deno_core::op2;
use deno_error::JsErrorBox;

/// Input is a "binary string": each code unit must be < 256 and maps to one byte.
#[op2]
#[string]
pub fn op_chq_btoa(#[string] input: String) -> Result<String, JsErrorBox> {
    let mut bytes = Vec::with_capacity(input.len());
    for ch in input.chars() {
        let code = ch as u32;
        if code > 0xFF {
            return Err(JsErrorBox::new(
                "InvalidCharacterError",
                "The string to be encoded contains characters outside of the Latin1 range",
            ));
        }
        bytes.push(code as u8);
    }
    Ok(STANDARD.encode(bytes))
}

#[op2]
#[string]
pub fn op_chq_atob(#[string] input: String) -> Result<String, JsErrorBox> {
    // The HTML spec strips ASCII whitespace before decoding.
    let cleaned: String = input.chars().filter(|c| !c.is_ascii_whitespace()).collect();
    let bytes = STANDARD
        .decode(cleaned.as_bytes())
        .or_else(|_| {
            // Padding is optional in the spec's decoder.
            base64::engine::general_purpose::STANDARD_NO_PAD.decode(cleaned.as_bytes())
        })
        .map_err(|_| {
            JsErrorBox::new(
                "InvalidCharacterError",
                "The string to be decoded is not correctly encoded",
            )
        })?;
    // Each byte becomes one code unit, mirroring `btoa`'s inverse.
    Ok(bytes.iter().map(|&b| b as char).collect())
}
