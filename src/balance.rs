use solana_client::rpc_client::RpcClient;
use solana_sdk::pubkey::Pubkey;
use spl_associated_token_account::get_associated_token_address;
use spl_token::state::Account as TokenAccount;
use solana_sdk::program_pack::Pack;

use crate::config::OreConfig;
use crate::keypair::{load_keypair, vector_pda};

const ORE_MINT: &str = "oreoU2P8bN6jkk3jbaiVxYnG1dCXcYxwhwyK9jSybcp";
const ORE_DECIMALS: u8 = 11;

pub fn run() -> anyhow::Result<()> {
    let cfg = OreConfig::load();
    let kp_path = cfg.keypair_path();

    if !kp_path.exists() {
        anyhow::bail!("No keypair found at {}. Run `ore init` first.", kp_path.display());
    }

    let (pk, _sk) = load_keypair(&kp_path)?;
    let (vector_addr, _bump) = vector_pda(&pk);
    let vector_pubkey = Pubkey::new_from_array(vector_addr.to_bytes());
    let ore_mint = ORE_MINT.parse::<Pubkey>()?;
    let ata = get_associated_token_address(&vector_pubkey, &ore_mint);

    let rpc = RpcClient::new(cfg.rpc_url());
    match rpc.get_account(&ata) {
        Ok(account) => {
            let token_account = TokenAccount::unpack(&account.data)?;
            let amount = token_account.amount;
            let whole = amount / 10u64.pow(ORE_DECIMALS as u32);
            let frac = amount % 10u64.pow(ORE_DECIMALS as u32);
            if frac == 0 {
                println!("{whole} ORE");
            } else {
                let frac_str = format!("{frac:0>width$}", width = ORE_DECIMALS as usize);
                let trimmed = frac_str.trim_end_matches('0');
                println!("{whole}.{trimmed} ORE");
            }
        }
        Err(_) => {
            println!("0 ORE (token account not found — run `ore init` first)");
        }
    }

    Ok(())
}
