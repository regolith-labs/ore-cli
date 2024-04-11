use std::{fs, io, path::PathBuf, time::Duration};

use cached::proc_macro::cached;
use ore::PROOF;
use solana_program::pubkey::Pubkey;

pub fn tunnel_keypair_filepath(id: u64) -> io::Result<PathBuf> {
    let home_dir = dirs::home_dir().expect("Home directory not found.");
    let ore_path = home_dir.join(".config").join("ore");
    fs::create_dir_all(&ore_path)?;
    Ok(ore_path.join(format!("tunnel-{}.json", id)))
}

pub async fn sleep_ms(ms: u64) {
    tokio::time::sleep(Duration::from_millis(ms)).await
}

#[cached]
pub fn proof_address(authority: Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[PROOF, authority.as_ref()], &ore::ID).0
}
