use ore::{state::Bus, utils::AccountDeserialize, BUS_ADDRESSES, TOKEN_DECIMALS};

use crate::Miner;

impl Miner {
    pub async fn busses(&self) {
        let client = self.rpc_client.clone();
        let mut max_bus: Option<Bus> = None;
        let mut max_rewards = 0.0;

        for address in BUS_ADDRESSES.iter() {
            let data = client.get_account_data(address).await.unwrap();
            match Bus::try_from_bytes(&data) {
                Ok(bus) => {
                    let rewards = (bus.rewards as f64) / 10f64.powf(TOKEN_DECIMALS as f64);
                    println!("Bus {}: {:} ORE", bus.id, rewards);
                    if rewards > max_rewards {
                        max_rewards = rewards;
                        max_bus = Some(*bus);
                    }
                }
                Err(_) => {}
            }
        }

        if let Some(bus) = max_bus {
            let max_rewards = (bus.rewards as f64) / 10f64.powf(TOKEN_DECIMALS as f64);
            println!("Bus with the most rewards: Bus {}: {:} ORE", bus.id, max_rewards);
        }
    }
}

