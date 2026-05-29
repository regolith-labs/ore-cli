use solana_sdk::pubkey::Pubkey;

use crate::config::OreConfig;
use crate::keypair::{load_keypair, vector_pda};

const ORE_MINT: &str = "oreoU2P8bN6jkk3jbaiVxYnG1dCXcYxwhwyK9jSybcp";

pub fn run() -> anyhow::Result<()> {
    let cfg = OreConfig::load();
    let kp_path = cfg.keypair_path();

    if !kp_path.exists() {
        anyhow::bail!("No keypair found at {}. Run `ore init` first.", kp_path.display());
    }

    let (pk, _sk) = load_keypair(&kp_path)?;
    let (vector_addr, _bump) = vector_pda(&pk);
    let vector_pubkey = Pubkey::new_from_array(vector_addr.to_bytes());

    println!("{vector_pubkey}");

    let solana_pay_url = format!("solana:{vector_pubkey}?spl-token={ORE_MINT}");
    println!();
    qr2term::print_qr(&solana_pay_url)?;

    Ok(())
}
