use ore::{BUS_ADDRESSES, state::Bus, utils::AccountDeserialize};
use solana_client::{client_error::Result, nonblocking::rpc_client::RpcClient};
use solana_sdk::commitment_config::CommitmentConfig;

use crate::Miner;

impl Miner {
    pub async fn busses(&self) {
        let client =
            RpcClient::new_with_commitment(self.cluster.clone(), CommitmentConfig::finalized());
        for address in BUS_ADDRESSES.iter() {
            if let Ok(data) = client.get_account_data(address).await {
                if let Ok(bus) = Bus::try_from_bytes(&data) {
                    println!("Bus {}: {:} ORE", bus.id, bus.rewards);
                }
            }
        }
    }

    pub async fn get_bus(&self, id: usize) -> Result<Bus> {
        let client =
            RpcClient::new_with_commitment(self.cluster.clone(), CommitmentConfig::finalized());
        // let data = client.get_account_data(&BUS_ADDRESSES[id]).await.unwrap();
        let data = match client.get_account_data(&BUS_ADDRESSES[id]).await {
            Ok(data) => data,
            Err(e) => return Err(e.into()), // 将错误转换为函数返回的错误类型
        };
        Ok(*Bus::try_from_bytes(&data).unwrap())
    }
}
