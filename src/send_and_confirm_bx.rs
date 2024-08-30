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

use crate::{
    send_and_confirm::{log_error, log_warning, ComputeBudget},
    utils::get_latest_blockhash_with_retries,
    Miner,
};
use tokio::time::{sleep, Duration};

const GATEWAY_RETRIES: usize = 150;
const GATEWAY_DELAY: u64 = 0;
const CONFIRM_DELAY: u64 = 500;
const CONFIRM_RETRIES: usize = 12;
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
        }

        final_ixs.extend_from_slice(ixs);

        let mut attempts = 0;
        let mut signature: Option<Signature> = None;
        let mut tx: Transaction =
            Transaction::new_with_payer(&final_ixs, Some(&fee_payer.pubkey()));
        let mut skip_submit = false;

        loop {
            progress_bar.println(format!("Attempt {} of {}", attempts + 1, GATEWAY_RETRIES));
            if attempts > GATEWAY_RETRIES {
                return Err(ClientError::from(ClientErrorKind::Custom(
                    "Max gateway retries reached".into(),
                )));
            }

            if attempts % 10 == 0 {
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

                let (hash, _slot) = get_latest_blockhash_with_retries(&self.rpc_client).await?;
                if signer.pubkey() == fee_payer.pubkey() {
                    tx.sign(&[&signer], hash);
                } else {
                    tx.sign(&[&signer, &fee_payer], hash);
                }

                skip_submit = false;
            }

            if !skip_submit {
                let tx_data = base64::prelude::BASE64_STANDARD.encode(
                    bincode::serialize(&tx).map_err(|e| {
                        progress_bar.println("Failed to serialize TX");
                        ClientError::from(ClientErrorKind::Custom(format!(
                            "Serialization error: {}",
                            e
                        )))
                    })?,
                );

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

                let client = reqwest::Client::new();

                let response = client.post(BLOXROUTE_URL).json(&request).send().await;

                let (status, response_text) = match response {
                    Ok(response) => {
                        let status = response.status();
                        match response.text().await {
                            Ok(text) => (status, text),
                            Err(e) => {
                                progress_bar.println(format!(
                                    "Failed to get response text: {}. Continuing...",
                                    e
                                ));
                                (status, String::from("{}"))
                            }
                        }
                    }
                    Err(e) => {
                        progress_bar
                            .println(format!("Bloxroute request error: {}. Continuing...", e));
                        (
                            reqwest::StatusCode::INTERNAL_SERVER_ERROR,
                            String::from("{}"),
                        )
                    }
                };

                println!("bloxroute response status: {}", status);

                let json_response: Value = serde_json::from_str(&response_text).map_err(|e| {
                    progress_bar.println(format!("Failed to get parse reponse json: {}", e));
                    ClientError::from(ClientErrorKind::Custom(format!(
                        "JSON parsing error: {}",
                        e
                    )))
                })?;

                if status.is_success() {
                    let signature_str = json_response["signature"].as_str().ok_or_else(|| {
                        progress_bar.println(format!("Failed to get signature"));
                        ClientError::from(ClientErrorKind::Custom(
                            "Signature not found in response".to_string(),
                        ))
                    })?;
                    signature = Some(Signature::from_str(signature_str).map_err(|e| {
                        progress_bar.println(format!("Failed to get parse signature: {}", e));
                        ClientError::from(ClientErrorKind::Custom(format!(
                            "Invalid signature: {}",
                            e
                        )))
                    })?);
                    skip_submit = true;
                } else {
                    let should_retry_rpc = match &json_response.get("code") {
                        Some(Value::Number(n)) => {
                            if let Some(code) = n.as_u64() {
                                if code == 6 {
                                    progress_bar.println(
                                        "Transaction already submitted. Skipping submission...",
                                    );
                                    skip_submit = true;
                                    false // Don't retry with RPC if code is 6
                                } else {
                                    true // Retry with RPC for any other code
                                }
                            } else {
                                true // Retry with RPC if code is not a u64
                            }
                        }
                        _ => true, // Retry with RPC if there's no 'code' field or it's not a number
                    };

                    if should_retry_rpc {
                        let send_cfg = RpcSendTransactionConfig {
                            skip_preflight: true,
                            preflight_commitment: Some(CommitmentLevel::Confirmed),
                            encoding: Some(UiTransactionEncoding::Base64),
                            max_retries: Some(0),
                            min_context_slot: None,
                        };

                        match self
                            .jito_client
                            .send_transaction_with_config(&tx, send_cfg)
                            .await
                        {
                            Ok(sig) => {
                                signature = Some(sig);
                                skip_submit = true;
                                progress_bar.println(format!(
                                    "Transaction sent via Jito. Signature: {}",
                                    sig
                                ));
                            }
                            Err(e) => {
                                if signature.is_none() {
                                    match self
                                        .rpc_client
                                        .send_transaction_with_config(&tx, send_cfg)
                                        .await
                                    {
                                        Ok(sig) => {
                                            signature = Some(sig);
                                            skip_submit = true;
                                            progress_bar.println(format!(
                                                "Transaction sent via fallback RPC. Signature: {}",
                                                sig
                                            ));
                                        }
                                        Err(e) => {
                                            progress_bar.println(format!(
                                                "Failed to send transaction via fallback RPC: {}",
                                                e
                                            ));
                                        }
                                    }
                                    progress_bar.println(format!(
                                        "Fallback rpc error: {}",
                                        e
                                    ))
                                }
                            }
                        }
                    }
                }
            } else {
                progress_bar.println(format!(
                    "Skipping BLXR Endpoint: Active sig: {:?}",
                    signature
                ));
            }

            if let Some(sig) = signature {
                'confirm: for _ in 0..CONFIRM_RETRIES {
                    sleep(Duration::from_millis(CONFIRM_DELAY)).await;
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
            sleep(Duration::from_millis(GATEWAY_DELAY)).await;
            attempts += 1;
        }
    }
}
