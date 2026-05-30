use solana_client::rpc_client::RpcClient;
use solana_sdk::signer::keypair::read_keypair_file;
use solana_sdk::signature::Signer;

use crate::config::OreConfig;

pub fn run() -> anyhow::Result<()> {
    let cfg = OreConfig::load();
    let fee_payer = read_keypair_file(cfg.fee_payer_path())
        .map_err(|e| anyhow::anyhow!("failed to read fee payer keypair: {e}"))?;
    let pubkey = fee_payer.pubkey();
    println!("{pubkey}");

    let rpc = RpcClient::new(cfg.rpc_url());
    let balance = rpc.get_balance(&pubkey)?;
    let sol = balance as f64 / 1_000_000_000.0;
    println!("{sol} SOL");

    Ok(())
}
