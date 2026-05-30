use ore_mint_api::consts::{MINT_ADDRESS, TOKEN_DECIMALS};
use ore_stake_api::prelude::*;
use solana_client::rpc_client::RpcClient;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::program_pack::Pack;
use solana_sdk::signature::Signer;
use solana_sdk::signer::keypair::read_keypair_file;
use spl_associated_token_account::get_associated_token_address;
use spl_token::state::Account as TokenAccount;
use steel::AccountDeserialize;

use crate::config::OreConfig;
use crate::keypair::{load_keypair, vector_pda};

fn format_ore(amount: u64) -> String {
    let whole = amount / 10u64.pow(TOKEN_DECIMALS as u32);
    let frac = amount % 10u64.pow(TOKEN_DECIMALS as u32);
    if frac == 0 {
        format!("{whole}")
    } else {
        let frac_str = format!("{frac:0>width$}", width = TOKEN_DECIMALS as usize);
        let trimmed = frac_str.trim_end_matches('0');
        format!("{whole}.{trimmed}")
    }
}

pub fn run() -> anyhow::Result<()> {
    let cfg = OreConfig::load();
    let kp_path = cfg.keypair_path();

    if !kp_path.exists() {
        anyhow::bail!("No keypair found at {}. Run `ore init` first.", kp_path.display());
    }

    let (pk, _sk) = load_keypair(&kp_path)?;
    let (vector_addr, _bump) = vector_pda(&pk);
    let vector_pubkey = Pubkey::new_from_array(vector_addr.to_bytes());
    let ore_mint = Pubkey::new_from_array(MINT_ADDRESS.to_bytes());

    let rpc = RpcClient::new(cfg.rpc_url());

    // ORE balance
    let ata = get_associated_token_address(&vector_pubkey, &ore_mint);
    let ore_balance = match rpc.get_account(&ata) {
        Ok(account) => {
            let token_account = TokenAccount::unpack(&account.data)?;
            format_ore(token_account.amount)
        }
        Err(_) => "0".to_string(),
    };

    // Stake balance and yield
    let stake_addr = ore_stake_api::state::stake_pda(vector_pubkey).0;
    let (stake_balance, stake_yield) = match rpc.get_account(&stake_addr) {
        Ok(stake_account) => {
            let mut stake = *Stake::try_from_bytes(&stake_account.data)?;
            let treasury_addr = ore_stake_api::state::treasury_pda().0;
            let vesting_addr = ore_stake_api::state::vesting_pda().0;
            let treasury_account = rpc.get_account(&treasury_addr)?;
            let vesting_account = rpc.get_account(&vesting_addr)?;
            let clock_data = rpc.get_account_data(&solana_sdk::sysvar::clock::ID)?;
            let mut treasury = *Treasury::try_from_bytes(&treasury_account.data)?;
            let mut vesting = *Vesting::try_from_bytes(&vesting_account.data)?;
            let clock: steel::Clock = bincode::deserialize(&clock_data)?;
            stake.update_rewards(&clock, &mut treasury, &mut vesting);
            (format_ore(stake.balance), format_ore(stake.rewards))
        }
        Err(_) => ("0".to_string(), "0".to_string()),
    };

    // Signer
    let fee_payer = read_keypair_file(cfg.fee_payer_path())
        .map_err(|e| anyhow::anyhow!("failed to read fee payer keypair: {e}"))?;
    let signer_pubkey = fee_payer.pubkey();
    let sol_balance = rpc.get_balance(&signer_pubkey)?;
    let sol = sol_balance as f64 / 1_000_000_000.0;

    println!("Address:  {vector_pubkey}");
    println!("Balance:  {ore_balance} ORE");
    println!("Stake:    {stake_balance} ORE");
    println!("Yield:    {stake_yield} ORE");
    println!("Signer:   {signer_pubkey}");
    println!("SOL:      {sol} SOL");

    Ok(())
}
