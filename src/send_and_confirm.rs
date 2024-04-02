use std::time::{Duration, Instant};

use solana_client::{
    client_error::{ClientError, ClientErrorKind, Result as ClientResult},
    connection_cache::ConnectionCache,
    nonblocking::tpu_client::TpuClient,
    rpc_config::RpcSimulateTransactionConfig,
    tpu_client::TpuClientConfig,
};
use solana_program::instruction::{Instruction, InstructionError};
use solana_sdk::{
    commitment_config::CommitmentConfig,
    compute_budget::ComputeBudgetInstruction,
    signature::{Signature, Signer},
    transaction::{Transaction, TransactionError},
};
use solana_transaction_status::UiTransactionEncoding;

use crate::Miner;

impl Miner {
    pub async fn send_and_confirm(&self, ixs: &[Instruction]) -> ClientResult<Signature> {
        let signer = self.signer();

        // Build tx
        let (hash, slot) = (&self.rpc_client)
            .get_latest_blockhash_with_commitment(CommitmentConfig::confirmed())
            .await
            .unwrap();

        let mut tx = Transaction::new_with_payer(ixs, Some(&signer.pubkey()));
        tx.sign(&[&signer], hash);

        // Sim and prepend cu ixs
        let sim_res = (&self.rpc_client)
            .simulate_transaction_with_config(
                &tx,
                RpcSimulateTransactionConfig {
                    sig_verify: false,
                    replace_recent_blockhash: false,
                    commitment: Some(CommitmentConfig::confirmed()),
                    encoding: Some(UiTransactionEncoding::Base64),
                    accounts: None,
                    min_context_slot: Some(slot),
                    inner_instructions: false,
                },
            )
            .await;
        if let Ok(sim_res) = sim_res {
            match sim_res.value.err {
                Some(err) => match err {
                    TransactionError::InstructionError(_, InstructionError::Custom(e)) => {
                        if e == 1 {
                            log::info!("Needs reset!");
                            return Err(ClientError {
                                request: None,
                                kind: ClientErrorKind::Custom("Needs reset".into()),
                            });
                        } else if e == 3 {
                            log::info!("Hash invalid!");
                            return Err(ClientError {
                                request: None,
                                kind: ClientErrorKind::Custom("Hash invalid".into()),
                            });
                        } else if e == 5 {
                            return Err(ClientError {
                                request: None,
                                kind: ClientErrorKind::Custom("Bus insufficient".into()),
                            });
                        } else {
                            return Err(ClientError {
                                request: None,
                                kind: ClientErrorKind::Custom("Sim failed".into()),
                            });
                        }
                    }
                    _ => {
                        return Err(ClientError {
                            request: None,
                            kind: ClientErrorKind::Custom("Sim failed".into()),
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
                kind: ClientErrorKind::Custom("Failed simulation".into()),
            });
        };

        eprintln!("{}", self.websocket_url.clone());

        let success = match &self.connection_cache {
            ConnectionCache::Quic(cache) => {
                TpuClient::new_with_connection_cache(
                    self.rpc_client.clone(),
                    &self.websocket_url,
                    TpuClientConfig::default(),
                    cache.clone(),
                )
                .await
                .expect("quic tpu client")
                .send_transaction(&tx)
                .await
            }
            ConnectionCache::Udp(cache) => {
                TpuClient::new_with_connection_cache(
                    self.rpc_client.clone(),
                    &self.websocket_url,
                    TpuClientConfig::default(),
                    cache.clone(),
                )
                .await
                .expect("udp tpu client")
                .send_transaction(&tx)
                .await
            }
        };

        assert!(success);
        let timeout = Duration::from_secs(5);
        let now = Instant::now();
        let signature = tx.signatures[0];
        loop {
            assert!(now.elapsed() < timeout);
            let statuses = &self
                .rpc_client
                .get_signature_statuses(&vec![signature])
                .await
                .unwrap();
            if statuses.value.first().is_some() {
                return Ok(signature);
            }
        }
    }
}
