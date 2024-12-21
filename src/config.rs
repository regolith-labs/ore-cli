use colored::Colorize;
use ore_api::consts::EPOCH_DURATION;

use crate::{utils::get_config, Miner};

impl Miner {
    pub async fn config(&self) {
        let config = get_config(&self.rpc_client).await;
        println!("{}: {}", "Last reset at".bold(), config.last_reset_at);
        println!("{}: {}", "Min difficulty".bold(), config.min_difficulty);
        println!("{}: {}", "Base reward rate".bold(), config.base_reward_rate);
        println!("{}: {} sec", "Epoch time".bold(), EPOCH_DURATION);
    }
}
