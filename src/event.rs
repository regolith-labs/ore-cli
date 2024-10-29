use base64::prelude::*;
use ore_api::{consts::TOKEN_DECIMALS, event::MineEvent};
use spl_token::{amount_to_ui_amount, amount_to_ui_amount_string};

use crate::{args::EventArgs, Miner};

impl Miner {
    pub async fn event(&self, args: EventArgs) {
        match BASE64_STANDARD.decode(args.data) {
            Err(err) => println!("Can't read bytes {:?}", err),
            Ok(bytes) => {
                let e = MineEvent::from_bytes(&bytes);
                println!(
                    "Balance: {}",
                    amount_to_ui_amount(e.balance, TOKEN_DECIMALS)
                );
                println!("Difficulty: {}", e.difficulty);
                println!("Last hash at: {}", e.last_hash_at);
                println!("Timing: {} sec", e.timing);
                println!(
                    "Reward: {} ORE",
                    amount_to_ui_amount(e.reward, TOKEN_DECIMALS)
                );
                println!(
                    "Boost 1: {} ORE",
                    amount_to_ui_amount_string(e.boost_1, TOKEN_DECIMALS)
                );
                println!(
                    "Boost 2: {} ORE",
                    amount_to_ui_amount_string(e.boost_2, TOKEN_DECIMALS)
                );
                println!(
                    "Boost 3: {} ORE",
                    amount_to_ui_amount_string(e.boost_3, TOKEN_DECIMALS)
                );
            }
        };
    }
}
