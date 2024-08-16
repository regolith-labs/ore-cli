use base64::Engine;
use chrono::Local;
use colored::Colorize;
use serde_json::json;
use solana_client::client_error::{ClientError, ClientErrorKind, Result as ClientResult};
use solana_program::instruction::Instruction;
use solana_rpc_client::spinner;
use solana_sdk::{
    compute_budget::ComputeBudgetInstruction, signature::Signature, signer::Signer,
    transaction::Transaction,
};
use std::str::FromStr;
use std::time::Duration;

use crate::{send_and_confirm::ComputeBudget, Miner};

const CONFIRM_DELAY: u64 = 500;
const CONFIRM_RETRIES: usize = 8;
const BLOXROUTE_URL: &str = "http://localhost:9000/api/v2/mine-ore";

impl Miner {
    pub async fn send_and_confirm_bx(
        &self,
        ixs: &[Instruction],
        compute_budget: ComputeBudget,
        skip_confirm: bool,
    ) -> ClientResult<Signature> {
        let progress_bar = spinner::new_progress_bar();
        let signer = self.signer();
        let fee_payer = self.fee_payer();

        // Build transaction
        let mut final_ixs = vec![];
        match compute_budget {
            ComputeBudget::Dynamic => {
                // TODO: Implement dynamic compute budget logic
            }
            ComputeBudget::Fixed(cus) => {
                final_ixs.push(ComputeBudgetInstruction::set_compute_unit_limit(cus))
            }
        }
        final_ixs.push(ComputeBudgetInstruction::set_compute_unit_price(
            self.priority_fee.unwrap_or(0),
        ));
        final_ixs.extend_from_slice(ixs);

        let mut tx = Transaction::new_with_payer(&final_ixs, Some(&fee_payer.pubkey()));
        let recent_blockhash = self.rpc_client.get_latest_blockhash().await?;

        if signer.pubkey() == fee_payer.pubkey() {
            tx.sign(&[&signer], recent_blockhash);
        } else {
            tx.sign(&[&signer, &fee_payer], recent_blockhash);
        }

        // Encode transaction
        let tx_data =
            base64::prelude::BASE64_STANDARD.encode(bincode::serialize(&tx).map_err(|e| {
                ClientError::from(ClientErrorKind::Custom(format!(
                    "Bincode serialization error: {}",
                    e
                )))
            })?);

        let body = json!({
            "transactions": vec![tx_data]
        });

        // Send transaction to custom endpoint
        progress_bar.set_message("Submitting transaction to custom endpoint...");
        let client = reqwest::Client::new();
        let response = client
            .post(BLOXROUTE_URL)
            .json(&body)
            .send()
            .await
            .map_err(|e| {
                ClientError::from(ClientErrorKind::Custom(format!("Request error: {}", e)))
            })?;

        let response_text = response.text().await.map_err(|e| {
            ClientError::from(ClientErrorKind::Custom(format!(
                "Response body error: {}",
                e
            )))
        })?;

        let response_json: serde_json::Value =
            serde_json::from_str(&response_text).map_err(|e| {
                ClientError::from(ClientErrorKind::Custom(format!(
                    "JSON parsing error: {}",
                    e
                )))
            })?;

        let signature = response_json["signature"].as_str().ok_or_else(|| {
            ClientError::from(ClientErrorKind::Custom(
                "Signature not found in response".to_string(),
            ))
        })?;

        let signature = Signature::from_str(signature).map_err(|e| {
            ClientError::from(ClientErrorKind::Custom(format!(
                "Signature parsing error: {}",
                e
            )))
        })?;

        // Skip confirmation if requested
        if skip_confirm {
            progress_bar.finish_with_message(format!("Sent: {}", signature));
            return Ok(signature);
        }

        // Confirm transaction
        progress_bar.set_message("Confirming transaction...");
        for _ in 0..CONFIRM_RETRIES {
            std::thread::sleep(Duration::from_millis(CONFIRM_DELAY));
            match self.rpc_client.get_signature_status(&signature).await {
                Ok(Some(status)) => {
                    if status.is_ok() {
                        let now = Local::now();
                        let formatted_time = now.format("%Y-%m-%d %H:%M:%S").to_string();
                        progress_bar.println(format!("  Timestamp: {}", formatted_time));
                        progress_bar.finish_with_message(format!(
                            "{} {}",
                            "OK".bold().green(),
                            signature
                        ));
                        return Ok(signature);
                    } else {
                        return Err(ClientError::from(ClientErrorKind::Custom(format!(
                            "Transaction failed: {:?}",
                            status
                        ))));
                    }
                }
                Ok(None) => continue,
                Err(err) => {
                    progress_bar.println(format!("  {} {}", "ERROR".bold().red(), err));
                }
            }
        }

        Err(ClientError::from(ClientErrorKind::Custom(
            "Transaction confirmation timeout".into(),
        )))
    }
}
