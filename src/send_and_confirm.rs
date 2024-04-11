use std::{
    io::{stdout, Write},
    time::{Duration, SystemTime, UNIX_EPOCH},
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
use tokio::{sync::mpsc::{self, Receiver, Sender}, time::sleep};

use crate::Miner;

const RPC_RETRIES: usize = 0;
const SIMULATION_RETRIES: usize = 4;
const GATEWAY_RETRIES: usize = 150;
const CONFIRM_RETRIES: usize = 1;

const CONFIRM_DELAY: u64 = 0;
const GATEWAY_DELAY: u64 = 300;

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
                kind: ClientErrorKind::Custom("Insufficient SOL balance".into()),
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
                    if let Some(err) = sim_res.value.err {
                        println!("Simulaton error: {:?}", err);
                        sim_attempts += 1;
                    } else if let Some(units_consumed) = sim_res.value.units_consumed {
                        if dynamic_cus {
                            println!("Dynamic CUs: {:?}", units_consumed);
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
                    println!("Simulaton error: {:?}", err);
                    sim_attempts += 1;
                }
            }

            // Abort if sim fails
            if sim_attempts.gt(&SIMULATION_RETRIES) {
                return Err(ClientError {
                    request: None,
                    kind: ClientErrorKind::Custom("Simulation failed".into()),
                });
            }
        }

        // Update hash before sending transactions
        let (hash, _slot) = client
            .get_latest_blockhash_with_commitment(self.rpc_client.commitment())
            .await
            .unwrap();

        // Submit tx
        tx.sign(&[&signer], hash);
        // let mut sigs = vec![];
        let mut attempts = 0;
        loop {
            println!("Attempt: {:?}", attempts);
            match client.send_transaction_with_config(&tx, send_cfg).await {
                Ok(sig) => {
                    println!("{:?}", sig);
                    // sigs.push(sig);

                    // Confirm tx
                    if skip_confirm {
                        return Ok(sig);
                    }
                    for _ in 0..CONFIRM_RETRIES {
                        std::thread::sleep(Duration::from_millis(CONFIRM_DELAY));
                        match client.get_signature_statuses(&[sig]).await {
                            Ok(signature_statuses) => {
                                println!("Confirmation: {:?}", signature_statuses.value[0]);
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
                                                    println!("Transaction landed!");
                                                    std::thread::sleep(Duration::from_millis(
                                                        GATEWAY_DELAY,
                                                    ));
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
                                println!("{:?}", err.kind().to_string());
                            }
                        }
                    }
                    println!("Transaction did not land");
                }

                // Handle submit errors
                Err(err) => {
                    println!("{:?}", err.kind().to_string());
                }
            }

            // Retry
            stdout.flush().ok();
            std::thread::sleep(Duration::from_millis(GATEWAY_DELAY));
            attempts += 1;
            if attempts > GATEWAY_RETRIES {
                return Err(ClientError {
                    request: None,
                    kind: ClientErrorKind::Custom("Max retries".into()),
                });
            }
        }
    }

        pub async fn send_and_confirm_v2(
        &self,
        ixs: &[Instruction],
        dynamic_cus: bool,
        send_interval: u64,
    ) -> Result<(Signature, u64), String> {
        let signer = self.signer();
        let client = self.rpc_client.clone();

        // Return error if balance is zero
        let balance = client.get_balance(&signer.pubkey()).await.unwrap();
        if balance <= 0 {
            return Err("Insufficient Sol balance".to_string());
           // return Err(ClientError {
           //     request: None,
           //     kind: ClientErrorKind::Custom("Insufficient SOL balance".into()),
           // });
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
                    if let Some(err) = sim_res.value.err {
                        println!("Simulaton error: {:?}", err);
                        sim_attempts += 1;
                    } else if let Some(units_consumed) = sim_res.value.units_consumed {
                        if dynamic_cus {
                            println!("Dynamic CUs: {:?}", units_consumed);
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
                    println!("Simulaton error: {:?}", err);
                    sim_attempts += 1;
                }
            }

            // Abort if sim fails
            if sim_attempts.gt(&SIMULATION_RETRIES) {
                return Err("Sim failed".to_string());
               // return Err(ClientError {
               //     request: None,
               //     kind: ClientErrorKind::Custom("Simulation failed".into()),
               // });
            }
        }

        // Update hash before sending transactions
        let (hash, last_valid_blockheight) = client
            .get_latest_blockhash_with_commitment(self.rpc_client.commitment())
            .await
            .unwrap();

        // Submit tx
        tx.sign(&[&signer], hash);
        let tx_signed_unix_ts = SystemTime::now().duration_since(UNIX_EPOCH).expect("Time went backwards").as_secs();

        // let mut sigs = vec![];

        // creates channel for sending the final tx result,
        //     Result will be Ok(sig) or Err("blockhash expired")
        let (tx_result_sender, mut tx_result_receiver): (
            Sender<Result<Signature, String>>,
            Receiver<Result<Signature, String>>,
        ) = mpsc::channel(100);

        // creates channel for getting sigs to confirm
        let (sig_checks_sender, mut sig_checks_receiver): (
            Sender<Result<Signature, String>>,
            Receiver<Result<Signature, String>>,
        ) = mpsc::channel(100);

        // confirmation checks thread
        let c_client = client.clone();
        let confirms_thread_handle = tokio::spawn(async move {
            let client = c_client;
            let mut sigs: Vec<Signature> = vec![];
            // receive sig_checks and add them to hashmap if new
            loop {
                if let Some(new_sig) = sig_checks_receiver.recv().await {
                    if let Ok(new_sig) = new_sig {
                        let mut is_new = true;
                        for sig in sigs.iter() {
                            if sig.to_string() == new_sig.to_string() {
                                is_new = false;
                            }
                        }

                        if is_new {
                            sigs.push(new_sig);
                        }
                    }
                }
                // really should only have one sig here though
                //for sig in sigs.iter {}
                // confirmation checks
                match client.get_signature_statuses(&sigs).await {
                    Ok(signature_statuses) => {
                        for signature_status in signature_statuses.value {
                            if let Some(signature_status) = signature_status.as_ref() {
                                if signature_status.confirmation_status.is_some() {
                                    let current_commitment =
                                        signature_status.confirmation_status.as_ref().unwrap();
                                    match current_commitment {
                                        TransactionConfirmationStatus::Processed => {}
                                        TransactionConfirmationStatus::Confirmed
                                        | TransactionConfirmationStatus::Finalized => {
                                            println!("Transaction landed!");
                                            let _ = tx_result_sender.send(Ok(sigs[0])).await;
                                            return;
                                        }
                                    }
                                }
                            }
                        }
                    }

                    // Handle confirmation errors
                    Err(err) => {
                        println!("{:?}", err.kind().to_string());
                    }
                }

                // hash expiration checks
                let current_blockheight = client.get_block_height().await.unwrap();
                if current_blockheight > last_valid_blockheight {
                    let err = Err("Last valid blockheight exceeded!".to_string());
                    let _ = tx_result_sender.send(err).await;
                    return;
                }

                // sleep 500ms to allow confirmations to potentially land
                sleep(Duration::from_millis(500)).await;
            }
        });

        let sender_thread_handle = tokio::spawn(async move {
            let sig_checks_sender = sig_checks_sender.clone();
            loop {
                let sig_checks_sender = sig_checks_sender.clone();
                let tx = tx.clone();
                let client = client.clone();
                tokio::spawn(async move {
                    // send off tx and get sig
                    let sig_checks_sender = sig_checks_sender.clone();

                    if let Ok(sig) = client.send_transaction_with_config(&tx, send_cfg).await {
                        match  sig_checks_sender.send(Ok(sig)).await {
                            Ok(_) => {
                            },
                            Err(_) => {
                                return;
                            }
                        }
                    } else {
                        // Program will still keep trying until last_valid_blockheight expires
                        // Transactions that get Err from RPC can still land.
                        // TODO: log errors to see what they are and if any other handling needs to
                        // be done.
                    };
                });
                // sleep 100ms (allowing 10 sends per second)
                sleep(Duration::from_millis(send_interval)).await;
            }
        });

        // wait for a tx result to come through
        let res = tx_result_receiver.recv().await.unwrap();
        confirms_thread_handle.abort();
        sender_thread_handle.abort();
        let tx_finished_unix_ts = SystemTime::now().duration_since(UNIX_EPOCH).expect("Time went backwards").as_secs();
        let tx_time_elapsed = tx_finished_unix_ts - tx_signed_unix_ts;

        match res {
            Ok(res) => {
                return Ok((res, tx_time_elapsed));
            }
            Err(_) => {
                return Err("Blockheight exceeded".to_string());
               // return Err(ClientError {
               //     request: None,
               //     kind: ClientErrorKind::Custom("Blockheight Exceeded for this signed transaction".into()),
               // });
            }
        }

        //return Err(ClientError {
        //    request: None,
        //    kind: ClientErrorKind::Custom("Max retries".into()),
        //});
    }

}
