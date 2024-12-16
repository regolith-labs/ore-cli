use std::str::FromStr;
use solana_program::pubkey::Pubkey;
use solana_sdk::signature::Signer;

use crate::{
    args::RotateArgs,
    send_and_confirm::ComputeBudget,
    Miner,
};

impl Miner {
    pub async fn rotate(&self, args: RotateArgs) {
        let signer = self.signer();
        
        // Parse mint address
        let mint_address = Pubkey::from_str(&args.mint)
            .expect("Failed to parse mint address");

        // Build and submit rotate transaction
        let ix = ore_boost_api::sdk::rotate(
            signer.pubkey(),
            mint_address,
        );

        let sig = self.send_and_confirm(&[ix], ComputeBudget::Fixed(50_000), false)
            .await
            .ok();

        println!("Signature: {}", sig.unwrap());
    }
}