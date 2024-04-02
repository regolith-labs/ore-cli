use ore::{state::Bus, utils::AccountDeserialize, BUS_ADDRESSES};

use crate::Miner;

impl Miner {
    pub async fn busses(&self) {
        for address in BUS_ADDRESSES.iter() {
            let data = &self.rpc_client.get_account_data(address).await.unwrap();
            match Bus::try_from_bytes(&data) {
                Ok(bus) => {
                    println!("Bus {}: {:} ORE", bus.id, bus.rewards);
                }
                Err(_) => {}
            }
        }
    }
}
