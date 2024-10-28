use ore_api::consts::ONE_ORE;

use crate::{
    utils::{amount_u64_to_string, get_config},
    Miner,
};

impl Miner {
    pub async fn rewards(&self) {
        let config = get_config(&self.rpc_client).await;
        let base_reward_rate = config.base_reward_rate;

        let mut s = format!(
            "{}: {} ORE",
            config.min_difficulty,
            amount_u64_to_string(base_reward_rate)
        )
        .to_string();
        for i in 1..40 {
            let reward_rate = base_reward_rate.saturating_mul(2u64.saturating_pow(i));
            s = format!(
                "{}\n{}: {} ORE",
                s,
                config.min_difficulty as u32 + i,
                if reward_rate > ONE_ORE {
                    "1.00000000000".to_string()
                } else {
                    amount_u64_to_string(reward_rate)
                }
            );
        }
        println!("{}", s);
    }
}
