use ore::{state::Bus, utils::AccountDeserialize, BUS_ADDRESSES, TOKEN_DECIMALS};

use crate::Miner;

impl Miner {
    pub async fn busses(&self) {
        let client = self.rpc_client.clone();
        let data = client.get_multiple_accounts(&BUS_ADDRESSES).await.unwrap();

        for (_address, account) in BUS_ADDRESSES.iter().zip(data.iter()) {
            match account {
                Some(account) => {
                    let data_bytes = &account.data[..]; // Extract data bytes
                    match Bus::try_from_bytes(data_bytes) {
                        Ok(bus) => {
                            let rewards = (bus.rewards as f64) / 10f64.powf(TOKEN_DECIMALS as f64);
                            println!("Bus {}: {} ORE", bus.id, rewards);
                        }
                        Err(_) => {}
                    }
                }
                None => {}
            }
        }
    }
}
