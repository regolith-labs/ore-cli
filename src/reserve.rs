use std::str::FromStr;
use solana_program::pubkey::Pubkey;
use solana_sdk::signature::Signer;

use crate::{
    args::ReserveArgs,
    send_and_confirm::ComputeBudget,
    Miner,
};

impl Miner {
    pub async fn reserve(&self, args: ReserveArgs) {
        let signer = self.signer();
        
        // Parse mint address
        let mint_address = Pubkey::from_str(&args.mint)
            .expect("Failed to parse mint address");

        // Build and submit reserve transaction
        let ix = ore_boost_api::sdk::reserve(
            signer.pubkey(),
            mint_address,
        );

        let sig = self.send_and_confirm(&[ix], ComputeBudget::Fixed(50_000), false)
            .await
            .ok();

        println!("Signature: {}", sig.unwrap());
    }
}