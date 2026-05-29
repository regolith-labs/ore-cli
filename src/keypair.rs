use std::fs;
use std::path::Path;

use pqcrypto_falcon::falcon512;
use pqcrypto_traits::sign::{PublicKey, SecretKey};
use sha2::{Digest, Sha256};
use vector_core::{find_vector_pda, FALCON512};

pub const FALCON512_WIRE_PUBKEY_LEN: usize = 897;
pub const FALCON512_SECRET_KEY_LEN: usize = 1281;

/// Load a Falcon-512 keypair from disk. File format: secret_key (2305) || public_key (897).
pub fn load_keypair(path: &Path) -> anyhow::Result<(falcon512::PublicKey, falcon512::SecretKey)> {
    let bytes = fs::read(path)?;
    if bytes.len() != FALCON512_SECRET_KEY_LEN + FALCON512_WIRE_PUBKEY_LEN {
        anyhow::bail!(
            "invalid keypair file: expected {} bytes, got {}",
            FALCON512_SECRET_KEY_LEN + FALCON512_WIRE_PUBKEY_LEN,
            bytes.len()
        );
    }
    let sk = falcon512::SecretKey::from_bytes(&bytes[..FALCON512_SECRET_KEY_LEN])
        .map_err(|_| anyhow::anyhow!("invalid Falcon-512 secret key"))?;
    let pk = falcon512::PublicKey::from_bytes(&bytes[FALCON512_SECRET_KEY_LEN..])
        .map_err(|_| anyhow::anyhow!("invalid Falcon-512 public key"))?;
    Ok((pk, sk))
}

/// Save a Falcon-512 keypair to disk. File format: secret_key (2305) || public_key (897).
pub fn save_keypair(
    path: &Path,
    pk: &falcon512::PublicKey,
    sk: &falcon512::SecretKey,
) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut bytes = Vec::with_capacity(FALCON512_SECRET_KEY_LEN + FALCON512_WIRE_PUBKEY_LEN);
    bytes.extend_from_slice(sk.as_bytes());
    bytes.extend_from_slice(pk.as_bytes());
    fs::write(path, &bytes)?;
    Ok(())
}

/// Get the 897-byte wire public key as a fixed-size array.
pub fn wire_pubkey(pk: &falcon512::PublicKey) -> [u8; FALCON512_WIRE_PUBKEY_LEN] {
    let bytes = pk.as_bytes();
    let mut out = [0u8; FALCON512_WIRE_PUBKEY_LEN];
    out.copy_from_slice(bytes);
    out
}

/// Compute the Falcon-512 identity: sha256(wire_pubkey).
pub fn falcon_identity(pk: &falcon512::PublicKey) -> [u8; 32] {
    let wire = wire_pubkey(pk);
    Sha256::digest(&wire).into()
}

/// Derive the Vector PDA address for a Falcon-512 public key.
pub fn vector_pda(pk: &falcon512::PublicKey) -> (solana_address::Address, u8) {
    let identity = falcon_identity(pk);
    find_vector_pda(&FALCON512, &identity)
}
