use colored::Colorize;
use ore_api::consts::EPOCH_DURATION;

use crate::{utils::{get_config, amount_u64_to_f64}, Miner};

impl Miner {
    pub async fn config(&self) {
        let config = get_config(&self.rpc_client).await;
        println!("{}: {}", "Last reset at".bold(), config.last_reset_at);
        println!("{}: {}", "Minimum difficulty".bold(), config.min_difficulty);
        println!("{}: {} ORE", "Base reward rate".bold(), amount_u64_to_f64(config.base_reward_rate));
        println!("{}: {} sec", "Epoch duration".bold(), EPOCH_DURATION);
    }
}
