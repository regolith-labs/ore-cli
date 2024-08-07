use base64::Engine;
use reqwest::Client;
use serde_json::json;
use solana_client::client_error::{ClientError, ClientErrorKind, Result as ClientResult};
use solana_sdk::{signature::Signature, transaction::Transaction};
use std::str::FromStr;

use crate::Miner;

impl Miner {
    pub async fn post_submit_v2(
        &self,
        transaction: &Transaction,
        skip_pre_flight: bool,
        use_staked_rpcs: bool,
        auth_token: &str,
    ) -> ClientResult<Signature> {
        let client = Client::new();
        let url = "https://api.blxrbdn.com";

        let tx_data = base64::prelude::BASE64_STANDARD.encode(
            bincode::serialize(transaction).map_err(|e| {
                ClientError::from(ClientErrorKind::Custom(format!(
                    "Bincode serialization error: {}",
                    e
                )))
            })?,
        );
        let body = json!({
            "transaction": {
                "content": tx_data
            },
            "skipPreFlight": skip_pre_flight,
            "useStakedRPCs": use_staked_rpcs,
        });

        let response: serde_json::Value = client
            .post(url)
            .json(&body)
            .header("Authorization", auth_token)
            .send()
            .await
            .map_err(|e| {
                ClientError::from(ClientErrorKind::Custom(format!("Request error: {}", e)))
            })?
            .json()
            .await
            .map_err(|e| {
                ClientError::from(ClientErrorKind::Custom(format!(
                    "JSON deserialization error: {}",
                    e
                )))
            })?;

        let signature = response["signature"].as_str().ok_or_else(|| {
            ClientError::from(ClientErrorKind::Custom(
                "Signature not found in response".to_string(),
            ))
        })?;

        Signature::from_str(signature).map_err(|e| {
            ClientError::from(ClientErrorKind::Custom(format!(
                "Signature parsing error: {}",
                e
            )))
        })
    }
}
