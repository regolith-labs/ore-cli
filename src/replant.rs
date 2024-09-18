use tokio::time::{sleep, Duration};

use colored::*;
use solana_sdk::signer::Signer;
use coal_api::consts::*;


use crate::{
    args::ReplantArgs,
    send_and_confirm::ComputeBudget,
    utils::{
		Resource,
		get_clock,
		get_config,
    },
    Miner,
};

impl Miner {
    pub async fn replant(&self, _args: ReplantArgs) {
		let signer = self.signer();

		println!("{} {}", "INFO".bold().green(), "Replanting wood...");

		loop {
			let config = get_config(&self.rpc_client, &Resource::Wood).await;
			let current_time = get_clock(&self.rpc_client).await.unix_timestamp;
			let epoch_duration = WOOD_EPOCH_DURATION as i64;
	
			let reset_time = config.last_reset_at().saturating_add(epoch_duration).saturating_sub(5);
			let time_until_reset = reset_time.saturating_sub(current_time).max(0);
	
			if time_until_reset > 0 {
				println!("Waiting for reset in {} seconds...", time_until_reset);
				sleep(Duration::from_secs(time_until_reset as u64)).await;
			}
	
			let compute_budget = 100_000;
			let ix = coal_api::instruction::reset_wood(signer.pubkey());
	
			// Submit transactions
			self.send_and_confirm(&[ix], ComputeBudget::Fixed(compute_budget), false).await.ok();
			println!("{} {}", "INFO".bold().green(), "Reset complete");
			sleep(Duration::from_secs(WOOD_EPOCH_DURATION as u64)).await;
		}
	}
}