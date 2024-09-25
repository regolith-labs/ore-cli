use std::str::FromStr;

use ore_boost_api::state::{boost_pda, stake_pda, Boost};
use ore_utils::AccountDeserialize;
use solana_program::{program_pack::Pack, pubkey::Pubkey};
use solana_sdk::signature::Signer;
use spl_token::state::Mint;

use crate::{args::StakeArgs, cu_limits::CU_LIMIT_CLAIM, send_and_confirm::ComputeBudget, Miner};

impl Miner {
    pub async fn stake(&self, args: StakeArgs) {
        // Parse mint address
        let mint_address = Pubkey::from_str(&args.mint).unwrap();

        // Get signer
        let signer = self.signer();
        let sender = match args.token_account {
            Some(address) => {
                Pubkey::from_str(&address).expect("Failed to parse token account address")
            }
            None => spl_associated_token_account::get_associated_token_address(
                &signer.pubkey(),
                &mint_address,
            ),
        };

        // Get token account
        let Ok(Some(token_account)) = self.rpc_client.get_token_account(&sender).await else {
            println!("Failed to fetch token account");
            return;
        };

        let Ok(mint_data) = self.rpc_client.get_account_data(&mint_address).await else {
            println!("Failed to fetch mint address");
            return;
        };
        let mint = Mint::unpack(&mint_data).unwrap();

        // Parse amount
        let amount: u64 = if let Some(amount) = args.amount {
            (amount * 10f64.powf(mint.decimals as f64)) as u64
        } else {
            u64::from_str(token_account.token_amount.amount.as_str())
                .expect("Failed to parse token balance")
        };

        // Get addresses
        let boost_address = boost_pda(mint_address).0;
        let stake_address = stake_pda(boost_address, signer.pubkey()).0;

        // Fetch boost
        let Ok(boost_account_data) = self.rpc_client.get_account_data(&boost_address).await else {
            println!("Failed to fetch boost account");
            return;
        };
        let _ = Boost::try_from_bytes(&boost_account_data).unwrap();

        // Open stake account, if needed
        if self
            .rpc_client
            .get_account_data(&stake_address)
            .await
            .is_err()
        {
            println!("Failed to fetch stake account");
            let ix = ore_boost_api::sdk::open(signer.pubkey(), mint_address);
            self.send_and_confirm(&[ix], ComputeBudget::Fixed(CU_LIMIT_CLAIM), false)
                .await
                .ok();
        }

        // Send tx
        let ix = ore_boost_api::sdk::deposit(signer.pubkey(), mint_address, amount);
        self.send_and_confirm(&[ix], ComputeBudget::Fixed(CU_LIMIT_CLAIM), false)
            .await
            .ok();
    }
}
