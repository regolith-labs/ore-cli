use crate::Miner;

use ore_api::consts::BUS_ADDRESSES;
use reqwest::Client;
use serde_json::{json, Value};

use solana_sdk::pubkey::Pubkey;
use std::{collections::HashMap, str::FromStr};

use solana_client::rpc_response::RpcPrioritizationFee;

use url::Url;
enum FeeStrategy {
    Helius,
    Triton,
    LOCAL,
    Alchemy,
    Quiknode,
}

impl Miner {
    pub async fn dynamic_fee(&self) -> Result<u64, String> {
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
        } else if host.contains("alchemy.com") {
            FeeStrategy::Alchemy
        } else if host.contains("quiknode.pro") {
            FeeStrategy::Quiknode
        } else if host.contains("rpcpool.com") {
            FeeStrategy::Triton
        } else {
            FeeStrategy::LOCAL
        };

        // Build fee estimate request
        let client = Client::new();
        let ore_addresses: Vec<String> = std::iter::once(ore_api::ID.to_string())
            .chain(BUS_ADDRESSES.iter().map(|pubkey| pubkey.to_string()))
            .collect();
        let body = match strategy {
            FeeStrategy::Helius => Some(json!({
                "jsonrpc": "2.0",
                "id": "priority-fee-estimate",
                "method": "getPriorityFeeEstimate",
                "params": [{
                    "accountKeys": ore_addresses,
                    "options": {
                        "recommended": true
                    }
                }]
            })),
            FeeStrategy::Alchemy => Some(json!({
                "jsonrpc": "2.0",
                "id": "priority-fee-estimate",
                "method": "getRecentPrioritizationFees",
                "params": [
                    ore_addresses
                ]
            })),
            FeeStrategy::Quiknode => Some(json!({
                "jsonrpc": "2.0",
                "id": "1",
                "method": "qn_estimatePriorityFees",
                "params": {
                    "account": "oreV2ZymfyeXgNgBdqMkumTqqAprVqgBWQfoYkrtKWQ",
                    "last_n_blocks": 100
                }
            })),
            FeeStrategy::Triton => Some(json!({
                "jsonrpc": "2.0",
                "id": "priority-fee-estimate",
                "method": "getRecentPrioritizationFees",
                "params": [
                    ore_addresses,
                    {
                        "percentile": 5000,
                    }
                ]
            })),
            FeeStrategy::LOCAL => None,
        };
        let response = if let Some(body) = body {
            let response: Value = client
                .post(rpc_url)
                .json(&body)
                .send()
                .await
                .unwrap()
                .json()
                .await
                .unwrap();
            response
        } else {
            Value::Null
        };
        // Parse response
        let calculated_fee = match strategy {
            FeeStrategy::Helius => response["result"]["priorityFeeEstimate"]
                .as_f64()
                .map(|fee| fee as u64)
                .ok_or_else(|| format!("Failed to parse priority fee response: {:?}", response)),
            FeeStrategy::Quiknode => response["result"]["per_compute_unit"]["medium"]
                .as_f64()
                .map(|fee| fee as u64)
                .ok_or_else(|| format!("Please enable the Solana Priority Fee API add-on in your QuickNode account.")),
            FeeStrategy::Alchemy => response["result"]
                .as_array()
                .and_then(|arr| {
                    Some(
                        arr.into_iter()
                            .map(|v| v["prioritizationFee"].as_u64().unwrap())
                            .collect::<Vec<u64>>(),
                    )
                })
                .and_then(|fees| {
                    Some(
                        ((fees.iter().sum::<u64>() as f32 / fees.len() as f32).ceil() * 1.2) as u64,
                    )
                })
                .ok_or_else(|| format!("Failed to parse priority fee response: {:?}", response)),
            FeeStrategy::Triton => {
                serde_json::from_value::<Vec<RpcPrioritizationFee>>(response["result"].clone())
                    .map(|prioritization_fees| {
                        estimate_prioritization_fee_micro_lamports(prioritization_fees)
                    })
                    .or_else(|error: serde_json::Error| {
                        Err(format!(
                            "Failed to parse priority fee response: {response:?}, error: {error}"
                        ))
                    })
            },
            FeeStrategy::LOCAL => self.dynamic_fee_strategy(18).await.or_else(|err| {
                Err(format!("Failed to parse priority fee response: {err}"))
            }),
        };

        // Check if the calculated fee is higher than max
        match calculated_fee {
            Err(err) => Err(err),
            Ok(fee) => {
                if let Some(max_fee) = self.priority_fee {
                    Ok(fee.min(max_fee))
                } else {
                    Ok(fee)
                }
            }
        }
    }
    pub async fn dynamic_fee_strategy(
        &self,
        difficulty: u32,
    ) -> Result<u64, Box<dyn std::error::Error>> {
        let client = self.rpc_client.clone();
        let pubkey = [
            "oreV2ZymfyeXgNgBdqMkumTqqAprVqgBWQfoYkrtKWQ",
            // "noop8ytexvkpCuqbf6FB89BSuNemHtPRqaNC31GWivW",
            "5HngGmYzvSuh3XyU11brHDpMTHXQQRQQT4udGFtQSjgR",
            "2oLNTQKRb4a2117kFi6BYTUDu3RPrMVAHFhCfPKMosxX",
        ];
        let address_strings = pubkey;

        // 将字符串转换为 Pubkey
        let addresses: Vec<Pubkey> = address_strings
            .into_iter()
            .map(|addr_str| Pubkey::from_str(addr_str).expect("Invalid address"))
            .collect();
        // 获取最近的优先级费用
        let recent_prioritization_fees = client.get_recent_prioritization_fees(&addresses).await?;

        if recent_prioritization_fees.is_empty() {
            println!("no recent prioritization fees");
            return Err("no recent prioritization fees".into());
        }
        let mut sorted_fees: Vec<_> = recent_prioritization_fees.into_iter().collect();
        sorted_fees.sort_by(|a, b| b.slot.cmp(&a.slot));

        let chunk_size = 150;
        let chunks: Vec<_> = sorted_fees.chunks(chunk_size).take(3).collect();
        let mut percentiles: HashMap<u8, u64> = HashMap::new();
        for (_, chunk) in chunks.iter().enumerate() {
            let fees: Vec<u64> = chunk.iter().map(|fee| fee.prioritization_fee).collect();
            percentiles = Self::calculate_percentiles(&fees);
        }
        let fee = if difficulty > 30 {
            *percentiles.get(&85).unwrap_or(&0)
        } else if difficulty > 22 {
            *percentiles.get(&80).unwrap_or(&0)
        } else if difficulty > 17 {
            *percentiles.get(&75).unwrap_or(&0)
        } else {
            *percentiles.get(&70).unwrap_or(&0)
        };
        println!("difficulty: {}, fee: {}", difficulty, fee);
        Ok(fee)
    }
    fn calculate_percentiles(fees: &[u64]) -> HashMap<u8, u64> {
        let mut sorted_fees = fees.to_vec();
        sorted_fees.sort_unstable();
        let len = sorted_fees.len();

        let percentiles = vec![10, 25, 50, 60, 70, 75, 80, 85, 90, 100];
        percentiles
            .into_iter()
            .map(|p| {
                let index = (p as f64 / 100.0 * len as f64).round() as usize;
                (p, sorted_fees[index.saturating_sub(1)])
            })
            .collect()
    }
}

/// Our estimate is the average over the last 20 slots
pub fn estimate_prioritization_fee_micro_lamports(
    prioritization_fees: Vec<RpcPrioritizationFee>,
) -> u64 {
    let prioritization_fees = prioritization_fees
        .into_iter()
        .rev()
        .take(20)
        .map(
            |RpcPrioritizationFee {
                 prioritization_fee, ..
             }| prioritization_fee,
        )
        .collect::<Vec<_>>();
    if prioritization_fees.is_empty() {
        panic!("Response does not contain any prioritization fees");
    }

    let prioritization_fee =
        prioritization_fees.iter().sum::<u64>() / prioritization_fees.len() as u64;

    prioritization_fee
}
