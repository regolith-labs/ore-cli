use std::str::FromStr;
use colored::Colorize;
use ore_boost_api::{consts::RESERVATION_INTERVAL, state::boost_pda};
use solana_program::pubkey::Pubkey;
use solana_sdk::signature::Signer;

use crate::{
    args::RotateArgs,
    send_and_confirm::ComputeBudget,
    Miner, utils::{get_boost, get_clock},
};

impl Miner {
    pub async fn rotate(&self, args: RotateArgs) {
        let signer = self.signer();
        
        // Parse mint address
        let mint_address = Pubkey::from_str(&args.mint)
            .expect("Failed to parse mint address");

        // Get boost
        let boost_address = boost_pda(mint_address).0;
        let boost = get_boost(&self.rpc_client, boost_address).await;
        let clock = get_clock(&self.rpc_client).await;

        // Check if enough time has passed since last rotation
        if clock.unix_timestamp < boost.reserved_at + RESERVATION_INTERVAL {
            println!( "{} Not enough time has passed since last rotation. Wait {} more seconds.",
            "WARNING".yellow(),
            RESERVATION_INTERVAL - (clock.unix_timestamp - boost.reserved_at));
            return;
        }

        // Build and submit rotate transaction
        let ix = ore_boost_api::sdk::rotate(
            signer.pubkey(),
            mint_address,
        );
        self.send_and_confirm(&[ix], ComputeBudget::Fixed(50_000), false)
            .await
            .ok();
    }
}