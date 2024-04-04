use std::{
    io::{stdout, Write},
    time::Duration,
};

use solana_client::{
    client_error::{ClientError, ClientErrorKind, Result as ClientResult},
    nonblocking::rpc_client::RpcClient,
    rpc_config::RpcSendTransactionConfig,
};
use solana_client::rpc_config::RpcSimulateTransactionConfig;
use solana_program::instruction::{Instruction, InstructionError};
use solana_sdk::{
    commitment_config::{CommitmentConfig, CommitmentLevel},
    signature::{Signature, Signer},
    transaction::Transaction,
};
use solana_sdk::compute_budget::ComputeBudgetInstruction;
use solana_sdk::transaction::TransactionError;
use solana_transaction_status::{TransactionConfirmationStatus, UiTransactionEncoding};

use crate::Miner;

const RPC_RETRIES: usize = 3;
const GATEWAY_RETRIES: usize = 5;
const CONFIRM_RETRIES: usize = 5;

impl Miner {
    pub async fn send_and_confirm(
        &self,
        ixs: &[Instruction],
        skip_confirm: bool,
    ) -> ClientResult<Signature> {
        let mut stdout = stdout();
        let signer = self.signer();
        let client =
            RpcClient::new_with_commitment(self.cluster.clone(), CommitmentConfig::processed());

        // Return error if balance is zero
        let balance = client
            .get_balance_with_commitment(&signer.pubkey(), CommitmentConfig::processed())
            .await
            ?;
        if balance.value <= 0 {
            return Err(ClientError {
                request: None,
                kind: ClientErrorKind::Custom("Insufficient SOL balance".into()),
            });
        }

        // Build tx
        let (mut hash, mut slot) = client
            .get_latest_blockhash_with_commitment(CommitmentConfig::processed())
            .await?;
        let mut send_cfg = RpcSendTransactionConfig {
            skip_preflight: true,
            preflight_commitment: Some(CommitmentLevel::Processed),
            encoding: Some(UiTransactionEncoding::Base64),
            max_retries: Some(RPC_RETRIES),
            min_context_slot: Some(slot),
        };
        let mut tx = Transaction::new_with_payer(ixs, Some(&signer.pubkey()));
        tx.sign(&[&signer], hash);



        // Sim and prepend cu ixs
        let sim_res = client
            .simulate_transaction_with_config(
                &tx,
                RpcSimulateTransactionConfig {
                    sig_verify: false,
                    replace_recent_blockhash: false,
                    commitment: Some(CommitmentConfig::processed()),
                    encoding: Some(UiTransactionEncoding::Base64),
                    accounts: None,
                    min_context_slot: Some(slot),
                    inner_instructions: false,
                },
            )
            .await;
        if let Ok(sim_res) = sim_res {
            match sim_res.value.err {
                Some(err) => return match err {
                    TransactionError::InstructionError(_, InstructionError::Custom(e)) => {
                        if e == 1 {
                            println!("Needs reset!");
                            Err(ClientError {
                                request: None,
                                kind: ClientErrorKind::Custom("Needs reset".into()),
                            })
                        } else if e == 3 {
                            println!("Hash invalid!");
                            Err(ClientError {
                                request: None,
                                kind: ClientErrorKind::Custom("Hash invalid".into()),
                            })
                        } else if e == 5 {
                            println!("Bus insufficient!");
                            Err(ClientError {
                                request: None,
                                kind: ClientErrorKind::Custom("Bus insufficient".into()),
                            })
                        } else {
                            println!("Sim failed!");
                            Err(ClientError {
                                request: None,
                                kind: ClientErrorKind::Custom("Sim failed".into()),
                            })
                        }
                    }
                    _ => {
                        Err(ClientError {
                            request: None,
                            kind: ClientErrorKind::Custom(format!("Sim failed: {:?}", err).into()),
                        })
                    }
                },
                None => {
                    let cu_budget_ix = ComputeBudgetInstruction::set_compute_unit_limit(
                        sim_res.value.units_consumed.unwrap() as u32 + 1000,
                    );
                    let cu_price_ix =
                        ComputeBudgetInstruction::set_compute_unit_price(self.priority_fee);
                    let mut final_ixs = vec![];
                    final_ixs.extend_from_slice(&[cu_budget_ix, cu_price_ix]);
                    final_ixs.extend_from_slice(ixs);
                    tx = Transaction::new_with_payer(&final_ixs, Some(&signer.pubkey()));
                    tx.sign(&[&signer], hash);
                }
            }
        } else {
            return Err(ClientError {
                request: None,
                kind: ClientErrorKind::Custom(format!("Sim failed: {:?}", sim_res).into()),
            });
        };


        // Submit tx
        let mut sigs = vec![];
        let mut attempts = 0;
        loop {
            println!("Attempt: {:?}", attempts);
            match client.send_transaction_with_config(&tx, send_cfg).await {
                Ok(sig) => {
                    sigs.push(sig);
                    println!("{:?}", sig);

                    // Confirm tx
                    if skip_confirm {
                        return Ok(sig);
                    }
                    for _ in 0..CONFIRM_RETRIES {
                        std::thread::sleep(Duration::from_millis(2000));
                        match client.get_signature_statuses(&sigs).await {
                            Ok(signature_statuses) => {
                                println!("Confirms: {:?}", signature_statuses);
                                for signature_status in signature_statuses.value {
                                    if let Some(signature_status) = signature_status.as_ref() {
                                        if signature_status.confirmation_status.is_some() {
                                            let current_commitment = signature_status
                                                .confirmation_status
                                                .as_ref().ok_or(ClientError {
                                                    request: None,
                                                    kind: ClientErrorKind::Custom("No status".into()),
                                                })?;
                                            match current_commitment {
                                                TransactionConfirmationStatus::Processed => {}
                                                TransactionConfirmationStatus::Confirmed
                                                | TransactionConfirmationStatus::Finalized => {
                                                    println!("Transaction landed!");
                                                    return Ok(sig);
                                                }
                                            }
                                        } else {
                                            println!("No status");
                                        }
                                    }
                                }
                            }

                            // Handle confirmation errors
                            Err(err) => {
                                println!("Error: {:?}", err);
                            }
                        }
                    }
                    println!("Transaction did not land");
                }

                // Handle submit errors
                Err(err) => {
                    println!("Error {:?}", err);
                }
            }
            stdout.flush().ok();

            // Retry
            std::thread::sleep(Duration::from_millis(200));
            (hash, slot) = client
                .get_latest_blockhash_with_commitment(CommitmentConfig::processed())
                .await?;
            send_cfg = RpcSendTransactionConfig {
                skip_preflight: true,
                preflight_commitment: Some(CommitmentLevel::Processed),
                encoding: Some(UiTransactionEncoding::Base64),
                max_retries: Some(RPC_RETRIES),
                min_context_slot: Some(slot),
            };
            tx.sign(&[&signer], hash);
            attempts += 1;
            if attempts > GATEWAY_RETRIES {
                return Err(ClientError {
                    request: None,
                    kind: ClientErrorKind::Custom("Max retries".into()),
                });
            }
        }
    }
}
