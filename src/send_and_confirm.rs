use solana_client::{
    client_error::{ClientError, ClientErrorKind, Result as ClientResult},
    connection_cache::ConnectionCache,
    nonblocking::tpu_client::TpuClient,
    rpc_config::RpcSendTransactionConfig,
    send_and_confirm_transactions_in_parallel::{
        send_and_confirm_transactions_in_parallel, SendAndConfirmConfig,
    },
    tpu_client::{TpuClientConfig, MAX_FANOUT_SLOTS},
};
use solana_program::instruction::{Instruction, InstructionError};
use solana_quic_client::{QuicConfig, QuicConnectionManager, QuicPool};
use solana_sdk::{
    commitment_config::{CommitmentConfig, CommitmentLevel},
    compute_budget::ComputeBudgetInstruction,
    signature::{Signature, Signer},
    transaction::{Transaction, TransactionError},
};

use crate::Miner;

pub(crate) const CU_LIMIT_REGISTER: u32 = 7660;
pub(crate) const CU_LIMIT_CLAIM: u32 = 11_000;
pub(crate) const CU_LIMIT_ATA: u32 = 24_000;
pub(crate) const CU_LIMIT_RESET: u32 = 12_200;
pub(crate) const CU_LIMIT_MINE: u32 = 3200;

// #[deprecated]
pub(crate) const CU_LIMIT_UNINFORMED_GUESSWORK_TO_MAKE_COMPILER_HAPPY: u32 = 30_000;

type QuicTpuClient = TpuClient<QuicPool, QuicConnectionManager, QuicConfig>;

impl Miner {
    pub async fn send_and_confirm(
        &self,
        ixs: &[Instruction],
        cu_limit: u32,
    ) -> ClientResult<Signature> {
        // self.send_and_confirm_with_rpc_client(ixs, cu_limit).await
        self.send_and_confirm_in_parallel(ixs, cu_limit).await
        // self.send_and_confirm_with_tpu_client(ixs, cu_limit).await
    }

    #[allow(dead_code)]
    pub async fn send_and_confirm_in_parallel(
        &self,
        ixs: &[Instruction],
        cu_limit: u32,
    ) -> ClientResult<Signature> {
        let signer = self.signer();

        let cu_budget_ix = ComputeBudgetInstruction::set_compute_unit_limit(cu_limit);
        let cu_price_ix = ComputeBudgetInstruction::set_compute_unit_price(self.priority_fee);
        let mut final_ixs = vec![];
        final_ixs.extend_from_slice(&[cu_budget_ix, cu_price_ix]);
        final_ixs.extend_from_slice(ixs);
        let tx = Transaction::new_with_payer(&final_ixs, Some(&signer.pubkey()));

        let tpu_client: Option<QuicTpuClient> = match &self.connection_cache {
            ConnectionCache::Quic(cache) => {
                let tpu_client = TpuClient::new_with_connection_cache(
                    self.rpc_client.clone(),
                    &self.websocket_url,
                    TpuClientConfig {
                        fanout_slots: MAX_FANOUT_SLOTS,
                    },
                    cache.clone(),
                )
                .await
                .expect("quic tpu client");
                Some(tpu_client)
            }
            _ => None,
        };

        let possible_tx_errors = send_and_confirm_transactions_in_parallel(
            self.rpc_client.clone(),
            tpu_client,
            &[tx.message],
            &[&signer],
            SendAndConfirmConfig {
                with_spinner: true,
                resign_txs_count: Some(50),
            },
        )
        .await
        .map_err(|e| {
            let kind = ClientErrorKind::Custom(e.to_string());
            ClientError {
                request: None,
                kind,
            }
        })?;

        for tx_error in possible_tx_errors.into_iter().flatten() {
            match tx_error {
                TransactionError::InstructionError(_, InstructionError::Custom(e)) => {
                    if e == 1 {
                        return Err(ClientError {
                            request: None,
                            kind: ClientErrorKind::Custom(format!("Needs reset e={}", e)),
                        });
                    } else if e == 3 {
                        return Err(ClientError {
                            request: None,
                            kind: ClientErrorKind::Custom(format!("Hash invalid e={}", e)),
                        });
                    } else if e == 5 {
                        return Err(ClientError {
                            request: None,
                            kind: ClientErrorKind::Custom(format!("Bus insufficient e={}", e)),
                        });
                    } else {
                        eprintln!("Sim failed e={}", e);
                        continue;
                        // return Err(ClientError {
                        //     request: None,
                        //     kind: ClientErrorKind::Custom(format!("Sim failed e={}", e)),
                        // });
                    }
                }
                _ => {
                    eprintln!("unknown tx_error={}", tx_error);
                    return Err(ClientError {
                        request: None,
                        kind: ClientErrorKind::Custom(tx_error.to_string()),
                    })
                }
            }
        }

        Ok(tx.signatures[0])
    }

    #[allow(dead_code)]
    pub async fn send_and_confirm_with_rpc_client(
        &self,
        ixs: &[Instruction],
        cu_limit: u32,
    ) -> ClientResult<Signature> {
        #[allow(deprecated)]
        let rpc_result = self
            .rpc_client
            .get_fees_with_commitment(CommitmentConfig::processed())
            .await?;

        let (blockhash, slot, _last_valid_block_height) = (
            rpc_result.value.blockhash,
            rpc_result.value.last_valid_block_height,
            rpc_result.context.slot,
        );

        let signer = self.signer();

        let cu_budget_ix = ComputeBudgetInstruction::set_compute_unit_limit(cu_limit);
        let cu_price_ix = ComputeBudgetInstruction::set_compute_unit_price(self.priority_fee);

        let mut final_ixs = vec![];
        final_ixs.extend_from_slice(&[cu_budget_ix, cu_price_ix]);
        final_ixs.extend_from_slice(ixs);

        let mut tx = Transaction::new_with_payer(&final_ixs, Some(&signer.pubkey()));
        tx.sign(&[&signer], blockhash);

        let signature = self
            .rpc_client
            .send_and_confirm_transaction_with_spinner_and_config(
                &tx,
                CommitmentConfig::confirmed(),
                RpcSendTransactionConfig {
                    max_retries: Some(0),
                    min_context_slot: Some(slot),
                    preflight_commitment: Some(CommitmentLevel::Processed),
                    skip_preflight: true,
                    ..Default::default()
                },
            )
            .await?;

        Ok(signature)
    }

    #[allow(dead_code)]
    pub async fn send_and_confirm_with_tpu_client(
        &self,
        ixs: &[Instruction],
        cu_limit: u32,
    ) -> ClientResult<Signature> {
        let signer = self.signer();

        let cu_budget_ix = ComputeBudgetInstruction::set_compute_unit_limit(cu_limit);
        let cu_price_ix = ComputeBudgetInstruction::set_compute_unit_price(self.priority_fee);

        let mut final_ixs = vec![];
        final_ixs.extend_from_slice(&[cu_budget_ix, cu_price_ix]);
        final_ixs.extend_from_slice(ixs);
        let tx = Transaction::new_with_payer(&final_ixs, Some(&signer.pubkey()));

        for retry in (0..20).rev() {
            let tx = tx.clone();

            let outcome = match &self.connection_cache {
                ConnectionCache::Quic(cache) => {
                    let tpu_client = TpuClient::new_with_connection_cache(
                        self.rpc_client.clone(),
                        &self.websocket_url,
                        TpuClientConfig::default(),
                        cache.clone(),
                    )
                    .await
                    .expect("quic tpu client");

                    tpu_client
                        .send_and_confirm_messages_with_spinner(&[tx.message], &[&signer])
                        .await
                }
                ConnectionCache::Udp(cache) => {
                    let tpu_client = TpuClient::new_with_connection_cache(
                        self.rpc_client.clone(),
                        &self.websocket_url,
                        TpuClientConfig::default(),
                        cache.clone(),
                    )
                    .await
                    .expect("udp tpu client");

                    tpu_client
                        .send_and_confirm_messages_with_spinner(&[tx.message], &[&signer])
                        .await
                }
            };

            if outcome.is_ok() {
                break;
            } else {
                eprintln!("outcome: {:?}", outcome);
                eprintln!("retrying: {}", retry);
                continue;
            }
        }

        Ok(tx.signatures[0])
    }
}
