// use std::io::{self};
use std::{
    io::{stdout, Write},
    time::Duration,
};

use solana_client::{
    client_error::{ClientError, ClientErrorKind, Result as ClientResult},
    rpc_config::{RpcSendTransactionConfig, RpcSimulateTransactionConfig},
};
use solana_program::instruction::Instruction;
use solana_sdk::{
    commitment_config::CommitmentLevel,
    compute_budget::ComputeBudgetInstruction,
    signature::{Signature, Signer},
    transaction::Transaction,
};
use solana_transaction_status::{TransactionConfirmationStatus, UiTransactionEncoding};

use crate::Miner;

const RPC_RETRIES: usize = 2;			// Default 0
const SIMULATION_RETRIES: usize = 1;	// Default 4
const CONFIRM_RETRIES: usize = 6;		// Default 4
const CONFIRM_DELAY: u64 = 5000;		// Default 5000
const GATEWAY_RETRIES: usize = 4;		// Default 4
const GATEWAY_DELAY: u64 = 2000;		// default 2000

impl Miner {
    pub async fn send_and_confirm(
        &self,
        ixs: &[Instruction],
        dynamic_cus: bool,
        skip_confirm: bool,
    ) -> ClientResult<Signature> {
        let mut stdout = stdout();
        let signer = self.signer();
        let client = self.rpc_client.clone();

        // Return error if balance is zero
        let balance = client.get_balance(&signer.pubkey()).await.unwrap();
        if balance <= 0 {
            return Err(ClientError {
                request: None,
                kind: ClientErrorKind::Custom("\n\t\t[ERROR]\tget_balance: Insufficient SOL balance".into()),
            });
        }

        // Build tx
        let (hash, slot) = client
            .get_latest_blockhash_with_commitment(self.rpc_client.commitment())
            .await
            .unwrap();
        let send_cfg = RpcSendTransactionConfig {
            skip_preflight: false,
            preflight_commitment: Some(CommitmentLevel::Finalized),
            encoding: Some(UiTransactionEncoding::Base64),
            max_retries: Some(RPC_RETRIES),
            min_context_slot: Some(slot),
        };
        let mut tx = Transaction::new_with_payer(ixs, Some(&signer.pubkey()));

        // Simulate tx
        let mut sim_attempts = 0;
		'simulate: loop {
            let sim_res = client
                .simulate_transaction_with_config(
                    &tx,
                    RpcSimulateTransactionConfig {
                        sig_verify: false,
                        replace_recent_blockhash: true,
                        commitment: Some(self.rpc_client.commitment()),
                        encoding: Some(UiTransactionEncoding::Base64),
                        accounts: None,
                        min_context_slot: Some(slot),
                        inner_instructions: false,
                    },
                )
                .await;
            match sim_res {
                Ok(sim_res) => {
					sim_attempts += 1;
                    if let Some(err) = sim_res.value.err {
                        eprintln!("\n\t\t[ERROR]\tSimulaton error: {:?}", err);
                    } else if let Some(units_consumed) = sim_res.value.units_consumed {
                        if dynamic_cus {
                            println!("\n\t[PASS]\tDynamic CUs: {:?}", units_consumed);
                            let cu_budget_ix = ComputeBudgetInstruction::set_compute_unit_limit(
                                units_consumed as u32 + 1000,
                            );
                            let cu_price_ix =
                                ComputeBudgetInstruction::set_compute_unit_price(self.priority_fee);
                            let mut final_ixs = vec![];
                            final_ixs.extend_from_slice(&[cu_budget_ix, cu_price_ix]);
                            final_ixs.extend_from_slice(ixs);
                            tx = Transaction::new_with_payer(&final_ixs, Some(&signer.pubkey()));
                        }
                        break 'simulate;
                    }
                }
                Err(err) => {
                    eprintln!("\n\t\t[ERRORA]\tSimulaton error: {:?}", err);
                    sim_attempts += 1;
                }
            }

            // Abort if sim fails
            if sim_attempts.gt(&SIMULATION_RETRIES) {
                return Err(ClientError {
                    request: None,
                    kind: ClientErrorKind::Custom("\n\t\t[ERROR]\tSimulation failed".into()),
                });
            }
        }

        // Submit tx
        tx.sign(&[&signer], hash);
        // let mut sigs = vec![];
        let mut attempts = 0;
        loop {
            attempts += 1;
            print!("Submission attempt {:?}\t", attempts);
			stdout.flush().unwrap();
			// Attempt to send the transaction
			match client.send_transaction_with_config(&tx, send_cfg).await {
                Ok(sig) => {
                    println!("TX ID: {:?}", sig);
					print!("\t\t\tAwaiting Transaction Confirmation...");
					stdout.flush().unwrap();
                    // sigs.push(sig);

                    // Confirm tx
                    if skip_confirm {
                        return Ok(sig);
                    }
					for retry_attempt in 0..CONFIRM_RETRIES {
						// Pause to give the confirmation time to succeed
						if retry_attempt == 0 {
							// Shorter first delay for first confirmation check
							std::thread::sleep(Duration::from_millis(1000));
						} else {
							std::thread::sleep(Duration::from_millis(CONFIRM_DELAY));
						}
                        match client.get_signature_statuses(&[sig]).await {
                            Ok(signature_statuses) => {
                                // print!(" [{:?}: {:?}]", retry_attempt+1, signature_statuses.value[0]);
								print!("[{:?}]", retry_attempt+1);
								stdout.flush().unwrap();
                                for signature_status in signature_statuses.value {
                                    if let Some(signature_status) = signature_status.as_ref() {
                                        if signature_status.confirmation_status.is_some() {
                                            let current_commitment = signature_status
                                                .confirmation_status
                                                .as_ref()
                                                .unwrap();
                                            match current_commitment {
                                                TransactionConfirmationStatus::Processed => {}
                                                TransactionConfirmationStatus::Confirmed
                                                | TransactionConfirmationStatus::Finalized => {
                                                    println!("\n\t\t[SUCCESS]\tTransaction landed!");
                                                    std::thread::sleep(Duration::from_millis(
                                                        GATEWAY_DELAY,
                                                    ));
                                                    return Ok(sig);
                                                }
                                            }

                                        } else {
                                            eprintln!("\n\t\t[ERROR]\tconfirmation_status: No status");
                                        }
                                    }
								}
                            }

                            // Handle confirmation errors
                            Err(err) => {
								eprintln!("\n\t\t[ERROR]\tget_signature_statuses: {:?}", err.kind().to_string());
                            }
                        }
                    }
                    eprintln!("\n\t\t[FAIL]\tTransaction did not land");
                }

                // Handle submit errors
                Err(err) => {
                    eprintln!("\n\t\t[ERROR]\tsend_transaction_with_config: {:?}", err.kind().to_string());
                }
            }

            stdout.flush().unwrap();
            // stdout.flush().ok();

            // If we get her then the transaction has not succeeded
			// std::thread::sleep(Duration::from_millis(GATEWAY_DELAY));
            if attempts >= GATEWAY_RETRIES {
                return Err(ClientError {
                    request: None,
                    kind: ClientErrorKind::Custom("Submitted up to max retries count with no success".into()),
                });
            }
        }
    }
}
