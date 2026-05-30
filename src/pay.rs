use ore_mint_api::consts::MINT_ADDRESS;
use solana_sdk::pubkey::Pubkey;

use crate::config::OreConfig;
use crate::keypair::{load_keypair, vector_pda};

pub fn run(amount: Option<String>) -> anyhow::Result<()> {
    let cfg = OreConfig::load();
    let kp_path = cfg.keypair_path();

    if !kp_path.exists() {
        anyhow::bail!("No keypair found at {}. Run `ore init` first.", kp_path.display());
    }

    let (pk, _sk) = load_keypair(&kp_path)?;
    let (vector_addr, _bump) = vector_pda(&pk);
    let vector_pubkey = Pubkey::new_from_array(vector_addr.to_bytes());

    let mint = Pubkey::new_from_array(MINT_ADDRESS.to_bytes());
    let solana_pay_url = match &amount {
        Some(amt) => format!("solana:{vector_pubkey}?amount={amt}&spl-token={mint}"),
        None => format!("solana:{vector_pubkey}?spl-token={mint}"),
    };

    // Scan to pay
    match &amount {
        Some(amt) => println!("Scan to pay {amt} ORE to {vector_pubkey}"),
        None => println!("Scan to pay {vector_pubkey}"),
    }

    println!();
    qr2term::print_qr(&solana_pay_url)?;

    Ok(())
}
