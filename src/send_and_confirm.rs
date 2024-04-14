// use std::io::{self};
use std::{
    io::{stdout, Write},
    time::Duration,
};
// use std::time::Instant;
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

const RPC_RETRIES: usize = 0;
const SIMULATION_RETRIES: usize = 4;
const GATEWAY_RETRIES: usize = 10;
const CONFIRM_RETRIES: usize = 3;

const CONFIRM_DELAY: u64 = 2500;
const GATEWAY_DELAY: u64 = 1000;

impl Miner {
    pub async fn send_and_confirm(
        &self,
        ixs: &[Instruction],
        dynamic_cus: bool,
        skip_confirm: bool,
    ) -> ClientResult<Signature> {
		// let startTime = Instant::now();
        let mut stdout = stdout();
        let signer = self.signer();
        let client = self.rpc_client.clone();

        // Return error if balance is zero
        let balance = client.get_balance(&signer.pubkey()).await.unwrap();
        if balance <= 0 {
            return Err(ClientError {
                request: None,
                kind: ClientErrorKind::Custom("\t[ERROR]\tget_balance: Insufficient SOL balance".into()),
            });
        }

        // Build tx
        let (_hash, slot) = client
            .get_latest_blockhash_with_commitment(self.rpc_client.commitment())
            .await
            .unwrap();
        let send_cfg = RpcSendTransactionConfig {
            skip_preflight: true,
            preflight_commitment: Some(CommitmentLevel::Confirmed),
            encoding: Some(UiTransactionEncoding::Base64),
            max_retries: Some(RPC_RETRIES),
            min_context_slot: None,
        };
        let mut tx = Transaction::new_with_payer(ixs, Some(&signer.pubkey()));

        // Simulate tx
        let mut sim_attempts = 0;
		'simulate: loop {
			sim_attempts += 1;
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
					print!("[Sim {:?}] ", sim_attempts);
					stdout.flush().unwrap();

					if let Some(err) = sim_res.value.err {
                        eprintln!("\t[ERROR]\tSimulaton error: {:?}", err);
                    } else {
						if let Some(units_consumed) = sim_res.value.units_consumed {
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
                }
                Err(err) => {
                    eprintln!("\t[ERROR]\tSimulaton error: {:?}", err);
                    sim_attempts += 1;
                }
            }

            // Abort if sim fails
            if sim_attempts.gt(&SIMULATION_RETRIES) {
                return Err(ClientError {
                    request: None,
                    kind: ClientErrorKind::Custom("\t[ERROR]\tSimulation failed".into()),
                });
            }
        }

        // Update hash before sending transactions
        let (hash, _slot) = client
            .get_latest_blockhash_with_commitment(self.rpc_client.commitment())
            .await
            .unwrap();

        // Sign the transaction
        tx.sign(&[&signer], hash);

        // let mut sigs = vec![];
        let mut attempts = 0;
        loop {
            attempts += 1;
			if attempts > 1 {
				print!("\t{:?}/{:?}:", attempts, GATEWAY_RETRIES);
				stdout.flush().unwrap();
			}
			// Attempt to send the transaction
			match client.send_transaction_with_config(&tx, send_cfg).await {
                Ok(sig) => {
					if attempts == 1 {
						println!("TX ID: {:?}", sig);
						print!("{:?}/{:?}:", attempts, GATEWAY_RETRIES);
					}
					stdout.flush().unwrap();
                    
                    // Confirm tx
                    if skip_confirm {
                        return Ok(sig);
                    }

					// Delay to prevent overloading your RPC & give the transaction a chance to process
					std::thread::sleep(Duration::from_millis(GATEWAY_DELAY));
		
					let mut log_progress = 10;
					for retry_attempt in 0..CONFIRM_RETRIES {
						// Pause between each confirmation check
						std::thread::sleep(Duration::from_millis(CONFIRM_DELAY));
		
		                match client.get_signature_statuses(&[sig]).await {
                            Ok(signature_statuses) => {
								log_progress-=1;
								if log_progress==0 {
									log_progress=10;
									print!("{:?}", retry_attempt+1);
								} else {
									print!(".");
								}
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
                                                    println!("[SUCCESS]\tTransaction landed!");
                                                    // std::thread::sleep(Duration::from_millis(
                                                    //     GATEWAY_DELAY,
                                                    // ));
                                                    return Ok(sig);
                                                }
                                            }

                                        } else {
                                            // eprintln!("[ERROR]\tconfirmation_status: No status");
                                            eprint!("[E1]");
                                        }
                                    }
								}
                            }

							Err(_err) => {	// Error Trap for get_signature_statuses
								eprint!("[E2]");
                            }
                        }
                    }
                    eprint!("[FAIL]");
                }

                Err(_err) => {	// Error trap for send_transaction_with_config
                    eprint!("[E3]");
                }
            }
            stdout.flush().unwrap();	// Just in case

            // If we get here then the transaction has not yet succeeded so try again
            if attempts >= GATEWAY_RETRIES {
                return Err(ClientError {
                    request: None,
                    kind: ClientErrorKind::Custom("Submitted up to max retries count with no success".into()),
                });
			}
			// Delay at end of attempt to prevent overloading your RPC
			std::thread::sleep(Duration::from_millis(GATEWAY_DELAY));
        }
    }
}
