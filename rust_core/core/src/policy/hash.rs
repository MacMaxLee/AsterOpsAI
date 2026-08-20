//! `parameters_hash` (TRS §25's approval binding). Unlike the audit chain
//! (`repository::audit`), which hand-rolls a length-prefixed encoding
//! because it can't trust field order from a `HashMap`, this hashes
//! `serde_json::to_vec` of the parameters `Value` directly — safe here
//! because this workspace doesn't enable serde_json's `preserve_order`
//! feature, so `serde_json::Map` (what backs `Value::Object`) is a
//! `BTreeMap` internally: key order in the serialized bytes is already
//! canonical (sorted), not insertion-order-dependent.

use sha2::{Digest, Sha256};

fn hex_encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

pub fn compute_parameters_hash(
    parameters: &serde_json::Value,
) -> Result<String, serde_json::Error> {
    let bytes = serde_json::to_vec(parameters)?;
    Ok(hex_encode(&Sha256::digest(&bytes)))
}
