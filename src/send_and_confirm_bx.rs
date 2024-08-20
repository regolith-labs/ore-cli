use base64::Engine;
use chrono::Local;
use colored::Colorize;
use ore_api::error::OreError;
use serde::Serialize;
use serde_json::Value;
use solana_client::client_error::{ClientError, ClientErrorKind, Result as ClientResult};
use solana_program::{
    instruction::{Instruction, InstructionError},
    pubkey::Pubkey,
    system_instruction,
};
use solana_rpc_client::spinner;
use solana_sdk::{
    commitment_config::CommitmentConfig,
    compute_budget::ComputeBudgetInstruction,
    signature::Signature,
    signer::Signer,
    transaction::{Transaction, TransactionError},
};
use std::str::FromStr;
use std::time::Duration;

use crate::{send_and_confirm::ComputeBudget, Miner};

const GATEWAY_RETRIES: usize = 150;
const GATEWAY_DELAY: u64 = 0;
const CONFIRM_DELAY: u64 = 750;
const CONFIRM_RETRIES: usize = 12;
const BLOXROUTE_URL: &str = "https://ore-ny.solana.dex.blxrbdn.com/api/v2/mine-ore";
// const BLOXROUTE_URL_LOCAL: &str = "http://localhost:9000/api/v2/mine-ore";

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
        skip_confirm: bool,
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
            let tip_instruction =
                system_instruction::transfer(&signer.pubkey(), &tip_pubkey, tip_amount);
            final_ixs.push(tip_instruction);
            progress_bar.println(format!("  Additional tip: {} lamports", tip_amount));
        } else {
            progress_bar.println("  No additional tip: Priority fee is zero");
        }

        final_ixs.extend_from_slice(ixs);

        let mut attempts = 0;
        let signature: Option<Signature>;

        loop {
            attempts += 1;
            if attempts > GATEWAY_RETRIES {
                return Err(ClientError::from(ClientErrorKind::Custom(
                    "Max gateway retries reached".into(),
                )));
            }

            progress_bar.set_message(format!("Submitting transaction... (attempt {})", attempts));

            // Prepare transaction
            if attempts % 10 == 1 {
                let recent_blockhash = self.rpc_client.get_latest_blockhash().await?;
                let mut tx = Transaction::new_with_payer(&final_ixs, Some(&fee_payer.pubkey()));
                if signer.pubkey() == fee_payer.pubkey() {
                    tx.sign(&[&signer], recent_blockhash);
                } else {
                    tx.sign(&[&signer, &fee_payer], recent_blockhash);
                }

                let tx_data = base64::prelude::BASE64_STANDARD.encode(
                    bincode::serialize(&tx).map_err(|e| {
                        ClientError::from(ClientErrorKind::Custom(format!(
                            "Bincode serialization error: {}",
                            e
                        )))
                    })?,
                );

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
                let response = client
                    .post(BLOXROUTE_URL)
                    .json(&request)
                    .send()
                    .await
                    .map_err(|e| {
                        ClientError::from(ClientErrorKind::Custom(format!("Request error: {}", e)))
                    })?;

                let status = response.status();
                let response_text = response.text().await.map_err(|e| {
                    ClientError::from(ClientErrorKind::Custom(format!(
                        "Error reading response body: {}",
                        e
                    )))
                })?;

                progress_bar.println(format!("Response status: {}", status));
                progress_bar.println(format!("Raw response: {}", response_text));

                if status.is_success() {
                    let json_response: Value =
                        serde_json::from_str(&response_text).map_err(|e| {
                            ClientError::from(ClientErrorKind::Custom(format!(
                                "Error parsing JSON response: {}",
                                e
                            )))
                        })?;

                    let signature_str = json_response["signature"].as_str().ok_or_else(|| {
                        ClientError::from(ClientErrorKind::Custom(
                            "Signature not found in response".to_string(),
                        ))
                    })?;

                    signature = Some(Signature::from_str(signature_str).map_err(|e| {
                        ClientError::from(ClientErrorKind::Custom(format!(
                            "Signature parsing error: {}",
                            e
                        )))
                    })?);

                    if skip_confirm {
                        progress_bar.finish_with_message(format!("Sent: {}", signature.unwrap()));
                        return Ok(signature.unwrap());
                    }

                    break;
                } else {
                    let json_response: Value =
                        serde_json::from_str(&response_text).unwrap_or_default();
                    if let Some(code) = json_response["code"].as_i64() {
                        match code {
                            6 => {
                                progress_bar.println(
                                    "Transaction already submitted. Moving to confirmation.",
                                );
                                if let Some(sig) = signature {
                                    break;
                                } else {
                                    progress_bar.println("No signature available for already submitted transaction. Retrying...");
                                }
                            }
                            _ => {
                                progress_bar.println(format!(
                                    "Bloxroute submission failed (code {}). Retrying with new blockhash...",
                                    code
                                ));
                            }
                        }
                    } else {
                        progress_bar
                            .println("Bloxroute submission failed. Retrying with new blockhash...");
                    }
                }
            }

            std::thread::sleep(Duration::from_millis(GATEWAY_DELAY));
        }

        // Confirmation stage
        let sig = signature.ok_or_else(|| {
            ClientError::from(ClientErrorKind::Custom(
                "No signature available for confirmation".into(),
            ))
        })?;

        progress_bar.set_message("Confirming transaction...");
        for attempt in 1..=CONFIRM_RETRIES {
            std::thread::sleep(Duration::from_millis(CONFIRM_DELAY));
            progress_bar.println(format!(
                "Confirmation attempt {} of {}",
                attempt, CONFIRM_RETRIES
            ));

            match self
                .rpc_client
                .get_signature_status_with_commitment(&sig, CommitmentConfig::confirmed())
                .await
            {
                Ok(Some(status)) => {
                    progress_bar.println(format!("  Received status: {:?}", status));
                    match status {
                        Ok(()) => {
                            let now = Local::now();
                            let formatted_time = now.format("%Y-%m-%d %H:%M:%S").to_string();
                            progress_bar.println(format!("  Timestamp: {}", formatted_time));
                            progress_bar.finish_with_message(format!(
                                "{} {}",
                                "OK".bold().green(),
                                sig
                            ));
                            return Ok(sig);
                        }
                        Err(err) => {
                            match err {
                                TransactionError::InstructionError(
                                    _,
                                    InstructionError::Custom(err_code),
                                ) => {
                                    if err_code == OreError::NeedsReset as u32 {
                                        progress_bar.println(
                                            "Needs reset. Restarting from the beginning...",
                                        );
                                        return Err(ClientError::from(ClientErrorKind::Custom(
                                            "Needs reset".into(),
                                        )));
                                    } else {
                                        progress_bar.println(format!("Transaction failed with instruction error. Error code: {}. Retrying...", err_code));
                                    }
                                }
                                _ => {
                                    progress_bar.println(format!("Transaction failed: {:?}. Restarting from the beginning...", err));
                                    return Err(ClientError::from(ClientErrorKind::Custom(
                                        format!("Transaction failed: {:?}", err),
                                    )));
                                }
                            }
                        }
                    }
                }
                Ok(None) => {
                    if attempt == CONFIRM_RETRIES {
                        return Err(ClientError::from(ClientErrorKind::Custom(
                            "Transaction not found after all retries".into(),
                        )));
                    } else {
                        progress_bar.println(
                            "  Transaction not yet processed. Continuing to next attempt.",
                        );
                    }
                }
                Err(err) => {
                    progress_bar.println(format!("  {} {}", "ERROR".bold().red(), err));
                    if attempt == CONFIRM_RETRIES {
                        return Err(ClientError::from(ClientErrorKind::Custom(
                            "Failed to get signature status after all retries".into(),
                        )));
                    } else {
                        progress_bar.println(
                            "  Failed to get signature status. Continuing to next attempt.",
                        );
                    }
                }
            }
        }
        Err(ClientError::from(ClientErrorKind::Custom(
            "Transaction confirmation failed after all retries".into(),
        )))
    }
}
