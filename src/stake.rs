use std::str::FromStr;

use solana_program::pubkey::Pubkey;
use solana_sdk::signature::Signer;

use crate::{
    args::StakeArgs, cu_limits::CU_LIMIT_CLAIM, send_and_confirm::ComputeBudget,
    utils::amount_f64_to_u64, Miner,
};

impl Miner {
    pub async fn stake(&self, args: StakeArgs) {
        // Get signer
        let signer = self.signer();
        let sender = match args.token_account {
            Some(address) => {
                Pubkey::from_str(&address).expect("Failed to parse token account address")
            }
            None => spl_associated_token_account::get_associated_token_address(
                &signer.pubkey(),
                &ore_api::consts::MINT_ADDRESS,
            ),
        };

        // Get token account
        let Ok(Some(token_account)) = self.rpc_client.get_token_account(&sender).await else {
            println!("Failed to fetch token account");
            return;
        };

        // Parse amount
        let amount: u64 = if let Some(amount) = args.amount {
            amount_f64_to_u64(amount)
        } else {
            u64::from_str(token_account.token_amount.amount.as_str())
                .expect("Failed to parse token balance")
        };

        // Send tx
        let ix = ore_api::instruction::stake(signer.pubkey(), sender, amount);
        self.send_and_confirm(&[ix], ComputeBudget::Fixed(CU_LIMIT_CLAIM), false)
            .await
            .ok();
    }
}
