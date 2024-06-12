use ore::{
    state::Bus,
    utils::AccountDeserialize,
    BUS_ADDRESSES,
    TOKEN_DECIMALS,
};

use crate::Miner;

impl Miner {
    pub async fn busses(&self) {
        let client = self.rpc_client.clone();
        let data = client.get_multiple_accounts(&BUS_ADDRESSES).await.unwrap();

        for (_address, account) in BUS_ADDRESSES.iter().zip(data.iter()) {
            if let Some(account) = account {
                let data_bytes = &account.data[..]; // Extract data bytes
                if let Ok(bus) = Bus::try_from_bytes(data_bytes) {
                    let rewards = (bus.rewards as f64) / 10f64.powf(TOKEN_DECIMALS as f64);
                    println!("Bus {}: {} ORE", bus.id, rewards);
                }
            }
        }
    }
}
