use base64::Engine;
use chrono::Local;
use colored::Colorize;
use ore_api::error::OreError;
use serde::Serialize;
use serde_json::Value;
use solana_client::{
    client_error::{ClientError, ClientErrorKind, Result as ClientResult},
    rpc_config::RpcSendTransactionConfig,
};
use solana_program::{instruction::Instruction, pubkey::Pubkey, system_instruction};
use solana_rpc_client::spinner;
use solana_sdk::{
    commitment_config::CommitmentLevel, compute_budget::ComputeBudgetInstruction,
    signature::Signature, signer::Signer, transaction::Transaction,
};
use solana_transaction_status::{TransactionConfirmationStatus, UiTransactionEncoding};
use std::str::FromStr;
use std::time::Duration;

use crate::{
    send_and_confirm::{log_error, log_warning, ComputeBudget},
    utils::get_latest_blockhash_with_retries,
    Miner,
};

const GATEWAY_RETRIES: usize = 150;
const GATEWAY_DELAY: u64 = 0;
const CONFIRM_DELAY: u64 = 500;
const CONFIRM_RETRIES: usize = 16;
const BLOXROUTE_URL: &str = "https://ore-ny.solana.dex.blxrbdn.com/api/v2/mine-ore";
// const BLOXROUTE_URL: &str = "http://localhost:9000/api/v2/mine-ore";

#[derive(Serialize)]
struct TransactionMessage {
    content: String,
    #[serde(rename = "isCleanup")]
    is_cleanup: bool,
}

#[derive(Serialize)]
struct PostSubmitRequest {
    transaction: TransactionMessage,
    #[serde(rename = "skipPreFlight")]
    skip_pre_flight: bool,
    #[serde(rename = "frontRunningProtection")]
    front_running_protection: Option<bool>,
    tip: Option<u64>,
    #[serde(rename = "useStakedRPCs")]
    use_staked_rpcs: Option<bool>,
    #[serde(rename = "fastBestEffort")]
    fast_best_effort: Option<bool>,
}

impl Miner {
    pub async fn send_and_confirm_bx(
        &self,
        ixs: &[Instruction],
        compute_budget: ComputeBudget,
    ) -> ClientResult<Signature> {
        let progress_bar = spinner::new_progress_bar();
        let signer = self.signer();
        let fee_payer = self.fee_payer();

        // Prepare instructions
        let mut final_ixs = vec![];
        match compute_budget {
            ComputeBudget::Dynamic => todo!("simulate tx"),
            ComputeBudget::Fixed(cus) => {
                final_ixs.push(ComputeBudgetInstruction::set_compute_unit_limit(cus))
            }
        }
        final_ixs.push(ComputeBudgetInstruction::set_compute_unit_price(
            self.priority_fee.unwrap_or(0),
        ));

        let tip = *self.tip.read().unwrap();
        if tip > 0 {
            let tip_amount = tip / 2;
            let tip_pubkey =
                Pubkey::from_str("HWEoBxYs7ssKuudEjzjmpfJVX7Dvi7wescFsVx2L5yoY").unwrap();
            final_ixs.push(system_instruction::transfer(
                &signer.pubkey(),
                &tip_pubkey,
                tip_amount,
            ));
            progress_bar.println(format!("  Additional tip: {} lamports", tip_amount));
        } else {
            progress_bar.println("  No additional tip: Priority fee is zero");
        }

        final_ixs.extend_from_slice(ixs);

        let mut attempts = 0;
        let mut signature: Option<Signature> = None;
        loop {
            attempts += 1;
            if attempts > GATEWAY_RETRIES {
                return Err(ClientError::from(ClientErrorKind::Custom(
                    "Max gateway retries reached".into(),
                )));
            }

            progress_bar.set_message(format!("Submitting transaction... (attempt {})", attempts));

            let mut tx = Transaction::new_with_payer(&final_ixs, Some(&fee_payer.pubkey()));

            if attempts % 10 == 0 {
                // Reset the compute unit price
                if self.dynamic_fee {
                    let fee = match self.dynamic_fee().await {
                        Ok(fee) => {
                            progress_bar.println(format!("  Priority fee: {} microlamports", fee));
                            fee
                        }
                        Err(err) => {
                            let fee = self.priority_fee.unwrap_or(0);
                            log_warning(
                                &progress_bar,
                                &format!(
                                    "{} Falling back to static value: {} microlamports",
                                    err, fee
                                ),
                            );
                            fee
                        }
                    };

                    final_ixs.remove(1);
                    final_ixs.insert(1, ComputeBudgetInstruction::set_compute_unit_price(fee));
                    tx = Transaction::new_with_payer(&final_ixs, Some(&fee_payer.pubkey()));
                }

                // Resign the tx
                let (hash, _slot) = get_latest_blockhash_with_retries(&self.rpc_client).await?;
                if signer.pubkey() == fee_payer.pubkey() {
                    tx.sign(&[&signer], hash);
                } else {
                    tx.sign(&[&signer, &fee_payer], hash);
                }
            }

            let tx_data =
                base64::prelude::BASE64_STANDARD.encode(bincode::serialize(&tx).map_err(|e| {
                    ClientError::from(ClientErrorKind::Custom(format!(
                        "Serialization error: {}",
                        e
                    )))
                })?);

            // Prepare request
            let request = PostSubmitRequest {
                transaction: TransactionMessage {
                    content: tx_data,
                    is_cleanup: false,
                },
                skip_pre_flight: true,
                front_running_protection: Some(true),
                tip: self.priority_fee,
                use_staked_rpcs: Some(true),
                fast_best_effort: Some(false),
            };

            // Submit transaction
            progress_bar.set_message("Submitting transaction to Bloxroute...");
            let client = reqwest::Client::new();
            let response = match client.post(BLOXROUTE_URL).json(&request).send().await {
                Ok(response) => response,
                Err(e) => {
                    progress_bar.println(format!("Bloxroute request error: {}. Retrying...", e));
                    std::thread::sleep(Duration::from_millis(GATEWAY_DELAY));
                    continue;
                }
            };

            let status = response.status();
            let response_text = response.text().await.map_err(|e| {
                ClientError::from(ClientErrorKind::Custom(format!(
                    "Failed to get response text: {}",
                    e
                )))
            })?;

            let json_response: Value = serde_json::from_str(&response_text).map_err(|e| {
                ClientError::from(ClientErrorKind::Custom(format!(
                    "JSON parsing error: {}",
                    e
                )))
            })?;

            progress_bar.println(format!(
                "Bloxroute Endpoint Response: {}",
                json_response
            ));

            if status.is_success() {
                let signature_str = json_response["signature"].as_str().ok_or_else(|| {
                    ClientError::from(ClientErrorKind::Custom(
                        "Signature not found in response".to_string(),
                    ))
                })?;
                signature = Some(Signature::from_str(signature_str).map_err(|e| {
                    ClientError::from(ClientErrorKind::Custom(format!("Invalid signature: {}", e)))
                })?);
            } else {
                match &json_response["code"] {
                    Value::Number(n) => {
                        if let Some(code) = n.as_u64() {
                            if code != 6 {
                                progress_bar.println("Sending via fallback RPC...");
                                // attempt to send via RPC client
                                let send_cfg = RpcSendTransactionConfig {
                                    skip_preflight: true,
                                    preflight_commitment: Some(CommitmentLevel::Confirmed),
                                    encoding: Some(UiTransactionEncoding::Base64),
                                    max_retries: Some(0),
                                    min_context_slot: None,
                                };

                                self.rpc_client
                                    .send_transaction_with_config(&tx, send_cfg)
                                    .await
                                    .ok();
                            }
                        }
                    }
                    _ => {}
                }
            }

            if let Some(sig) = signature {
                'confirm: for _ in 0..CONFIRM_RETRIES {
                    std::thread::sleep(Duration::from_millis(CONFIRM_DELAY));
                    match self.rpc_client.get_signature_statuses(&[sig]).await {
                        Ok(signature_statuses) => {
                            for status in signature_statuses.value {
                                if let Some(status) = status {
                                    if let Some(err) = status.err {
                                        match err {
                                            // Instruction error
                                            solana_sdk::transaction::TransactionError::InstructionError(_, err) => {
                                                match err {
                                                    // Custom instruction error, parse into OreError
                                                    solana_program::instruction::InstructionError::Custom(err_code) => {
                                                        match err_code {
                                                            e if e == OreError::NeedsReset as u32 => {
                                                                attempts = 0;
                                                                log_error(&progress_bar, "Needs reset. Retrying...", false);
                                                                break 'confirm;
                                                            },
                                                            _ => {
                                                                log_error(&progress_bar, &err.to_string(), true);
                                                                return Err(ClientError {
                                                                    request: None,
                                                                    kind: ClientErrorKind::Custom(err.to_string()),
                                                                });
                                                            }
                                                        }
                                                    },

                                                    // Non custom instruction error, return
                                                    _ => {
                                                        log_error(&progress_bar, &err.to_string(), true);
                                                        return Err(ClientError {
                                                            request: None,
                                                            kind: ClientErrorKind::Custom(err.to_string()),
                                                        });
                                                    }
                                                }
                                            },

                                            // Non instruction error, return
                                            _ => {
                                                log_error(&progress_bar, &err.to_string(), true);
                                                return Err(ClientError {
                                                    request: None,
                                                    kind: ClientErrorKind::Custom(err.to_string()),
                                                });
                                            }
                                        }
                                    } else if let Some(confirmation) = status.confirmation_status {
                                        match confirmation {
                                            TransactionConfirmationStatus::Processed => {}
                                            TransactionConfirmationStatus::Confirmed
                                            | TransactionConfirmationStatus::Finalized => {
                                                let now = Local::now();
                                                let formatted_time =
                                                    now.format("%Y-%m-%d %H:%M:%S").to_string();
                                                progress_bar.println(format!(
                                                    "  Timestamp: {}",
                                                    formatted_time
                                                ));
                                                progress_bar.finish_with_message(format!(
                                                    "{} {}",
                                                    "OK".bold().green(),
                                                    sig
                                                ));
                                                return Ok(sig);
                                            }
                                        }
                                    }
                                }
                            }
                        }

                        // Handle confirmation errors
                        Err(err) => {
                            log_error(&progress_bar, &err.kind().to_string(), false);
                        }
                    }
                }
            }

            // If we've exhausted all confirmation retries, continue to the next submission attempt
            progress_bar.println("Confirmation attempts exhausted. Retrying submission...");
            std::thread::sleep(Duration::from_millis(GATEWAY_DELAY));
        }
    }
}
