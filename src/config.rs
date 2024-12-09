use colored::Colorize;
use ore_api::consts::EPOCH_DURATION;
use ore_boost_api::state::{leaderboard_pda, Leaderboard};
use steel::AccountDeserialize;

use crate::{utils::get_config, Miner};

impl Miner {
    pub async fn config(&self) {
        let config = get_config(&self.rpc_client).await;
        println!("{}: {}", "Last reset at".bold(), config.last_reset_at);
        println!("{}: {}", "Min difficulty".bold(), config.min_difficulty);
        println!("{}: {}", "Base reward rate".bold(), config.base_reward_rate);
        println!("{}: {} sec", "Epoch time".bold(), EPOCH_DURATION);

        // Print leaderboard
        let leaderboard_pda = leaderboard_pda();
        match self.rpc_client.get_account(&leaderboard_pda.0).await {
            Ok(account) => {
                let leaderboard = Leaderboard::try_from_bytes(&account.data).unwrap();
                println!("\n{}", "Leaderboard:".bold());
                for (i, entry) in leaderboard.entries.iter().enumerate() {
                    if entry.score > 0 {
                        println!("{}. {} - Score: {}", i + 1, entry.address, entry.score);
                    }
                }
            }
            Err(_) => println!("Could not fetch leaderboard data")
        }
    }
}
