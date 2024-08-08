use crate::Miner;

use ore_api::consts::BUS_ADDRESSES;
use reqwest::Client;
use serde_json::{json, Value};
use solana_client::rpc_response::RpcPrioritizationFee;
use url::Url;

enum FeeStrategy {
    Helius,
    Triton,
}

impl Miner {
    pub async fn dynamic_fee(&self) -> Option<u64> {
        // Get url
        let rpc_url = self
            .dynamic_fee_url
            .clone()
            .unwrap_or(self.rpc_client.url());

        // Select fee estiamte strategy
        let host = Url::parse(&rpc_url)
            .unwrap()
            .host_str()
            .unwrap()
            .to_string();
        let strategy = if host.contains("helius-rpc.com") {
            FeeStrategy::Helius
        } else if host.contains("rpcpool.com") {
            FeeStrategy::Triton
        } else {
            return None;
        };

        // Build fee estimate request
        let client = Client::new();
        let ore_addresses: Vec<String> = std::iter::once(ore_api::ID.to_string())
            .chain(BUS_ADDRESSES.iter().map(|pubkey| pubkey.to_string()))
            .collect();
        let body = match strategy {
            FeeStrategy::Helius => {
                json!({
                    "jsonrpc": "2.0",
                    "id": "priority-fee-estimate",
                    "method": "getPriorityFeeEstimate",
                    "params": [{
                        "accountKeys": ore_addresses,
                        "options": {
                            "recommended": true
                        }
                    }]
                })
            }
            FeeStrategy::Triton => {
                json!({
                    "jsonrpc": "2.0",
                    "id": "priority-fee-estimate",
                    "method": "getRecentPrioritizationFees",
                    "params": [
                        ore_addresses,
                        {
                            "percentile": 5000,
                        }
                    ]
                })
            }
        };

        // Send request
        let response: Value = client
            .post(rpc_url)
            .json(&body)
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();

        // Parse response
        let calculated_fee = match strategy {
            FeeStrategy::Helius => response["result"]["priorityFeeEstimate"]
                .as_f64()
                .map(|fee| fee as u64)
                .ok_or_else(|| format!("Failed to parse priority fee. Response: {:?}", response))
                .unwrap(),
            FeeStrategy::Triton => {
                serde_json::from_value::<Vec<RpcPrioritizationFee>>(response["result"].clone())
                    .map(|arr| estimate_prioritization_fee_micro_lamports(&arr))
                    .or_else(|error| {
                        Err(format!(
                            "Failed to parse priority fee. Response: {response:?}, error: {error}"
                        ))
                    })
                    .unwrap()
            }
        };

        // Check if the calculated fee is higher than max
        if let Some(max_fee) = self.priority_fee {
            Some(calculated_fee.min(max_fee))
        } else {
            Some(calculated_fee)
        }
    }
}

/// Our estimate is the average over the last 20 slots
/// Take last 20 slots and average
pub fn estimate_prioritization_fee_micro_lamports(
    prioritization_fees: &[RpcPrioritizationFee],
) -> u64 {
    let prioritization_fees = prioritization_fees
        .iter()
        .rev()
        .take(20)
        .map(
            |RpcPrioritizationFee {
                 prioritization_fee, ..
             }| *prioritization_fee,
        )
        .collect::<Vec<_>>();
    if prioritization_fees.is_empty() {
        panic!("Response does not contain any prioritization fees");
    }

    let prioritization_fee =
        prioritization_fees.iter().sum::<u64>() / prioritization_fees.len() as u64;

    prioritization_fee
}
