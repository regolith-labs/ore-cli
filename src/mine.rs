use std::{
    sync::{Arc, RwLock},
    time::Instant,
};
use colored::*;
use drillx::{equix, Hash, Solution};
use ore_api::{
    consts::{BUS_ADDRESSES, BUS_COUNT, EPOCH_DURATION},
    state::{Bus, Config, Proof},
};
use ore_utils::AccountDeserialize;
use rand::Rng;
use solana_program::pubkey::Pubkey;
use solana_rpc_client::spinner;
use solana_sdk::signer::Signer;

use crate::{
    args::MineArgs,
    send_and_confirm::ComputeBudget,
    utils::{
        amount_u64_to_string, get_clock, get_config, get_updated_proof_with_authority, proof_pubkey,
    },
    Miner,
};

impl Miner {
    pub async fn mine(&self, args: MineArgs) {
        let signer = self.signer();
        self.open().await;
        self.check_num_cores(args.cores);

        let mut last_hash_at = 0;
        let mut last_balance = 0;
        loop {
            let config = get_config(&self.rpc_client).await;
            let proof = get_updated_proof_with_authority(&self.rpc_client, signer.pubkey(), last_hash_at).await;

            println!(
                "\n\nStake: {} ORE\n{}  Multiplier: {:12}x",
                amount_u64_to_string(proof.balance),
                if last_hash_at > 0 {
                    format!(
                        "  Change: {} ORE\n",
                        amount_u64_to_string(proof.balance.saturating_sub(last_balance))
                    )
                } else {
                    "".to_string()
                },
                calculate_multiplier(proof.balance, config.top_balance)
            );

            last_hash_at = proof.last_hash_at;
            last_balance = proof.balance;

            let cutoff_time = self.get_cutoff(proof, args.buffer_time).await;
            let solution = Self::find_hash_par(proof, cutoff_time, args.cores, config.min_difficulty as u32).await;

            let mut ixs = vec![ore_api::instruction::auth(proof_pubkey(signer.pubkey()))];
            let mut compute_budget = 500_000;
            if self.should_reset(config).await && rand::thread_rng().gen_range(0..100) == 0 {
                compute_budget += 100_000;
                ixs.push(ore_api::instruction::reset(signer.pubkey()));
            }

            ixs.push(ore_api::instruction::mine(
                signer.pubkey(),
                signer.pubkey(),
                self.find_bus().await,
                solution,
            ));

            self.send_and_confirm(&ixs, ComputeBudget::Fixed(compute_budget), false).await.ok();
        }
    }

    async fn find_hash_par(
        proof: Proof,
        cutoff_time: u64,
        cores: u64,
        min_difficulty: u32,
    ) -> Solution {
        let progress_bar = Arc::new(spinner::new_progress_bar());
        let global_best_difficulty = Arc::new(RwLock::new(0u32));
        progress_bar.set_message("Mining...");

        let core_ids = core_affinity::get_core_ids().unwrap();
        let total_cores = cores.min(core_ids.len() as u64);
        let workload_per_core = u64::MAX / total_cores;

        let handles: Vec<_> = core_ids
            .into_iter()
            .enumerate()
            .map(|(i, core_id)| {
                let global_best_difficulty = Arc::clone(&global_best_difficulty);
                let progress_bar = Arc::clone(&progress_bar);
                let mut memory = equix::SolverMemory::new();
                std::thread::spawn(move || {
                    if i as u64 >= total_cores {
                        return (0, 0, Hash::default());
                    }

                    if !core_affinity::set_for_current(core_id) {
                        eprintln!("Failed to set affinity for core {:?}", core_id);
                    }
                    
                    let timer = Instant::now();
                    let mut nonce = workload_per_core * i as u64;
                    let mut best_nonce = nonce;
                    let mut best_difficulty = 0;
                    let mut best_hash = Hash::default();

                    loop {
                        if let Ok(hx) = drillx::hash_with_memory(&mut memory, &proof.challenge, &nonce.to_le_bytes()) {
                            let difficulty = hx.difficulty();
                            if difficulty > best_difficulty {
                                best_nonce = nonce;
                                best_difficulty = difficulty;
                                best_hash = hx;

                                let mut best_global_difficulty = global_best_difficulty.write().unwrap();
                                if best_difficulty > *best_global_difficulty {
                                    *best_global_difficulty = best_difficulty;
                                }
                            }
                        }

                        // Check if global best difficulty meets or exceeds min_difficulty
                        if best_difficulty >= min_difficulty {
                            break;
                        }

                        if nonce % 100 == 0 {
                            let elapsed_secs = timer.elapsed().as_secs();
                            if elapsed_secs >= cutoff_time {
                                break;
                            }

                            if i == 0 {
                                let global_best_difficulty = *global_best_difficulty.read().unwrap();
                                progress_bar.set_message(format!(
                                    "Mining... (difficulty {}, time {})",
                                    global_best_difficulty,
                                    format_duration(cutoff_time.saturating_sub(elapsed_secs) as u32),
                                ));
                            }
                        }

                        nonce += 1;
                    }

                    (best_nonce, best_difficulty, best_hash)
                })
            })
            .collect();

        let mut best_nonce = 0;
        let mut best_difficulty = 0;
        let mut best_hash = Hash::default();
        for h in handles {
            if let Ok((nonce, difficulty, hash)) = h.join() {
                if difficulty > best_difficulty {
                    best_difficulty = difficulty;
                    best_nonce = nonce;
                    best_hash = hash;
                }
            }
        }

        progress_bar.finish_with_message(format!(
            "Best hash: {} (difficulty {})",
            bs58::encode(best_hash.h).into_string(),
            best_difficulty
        ));

        Solution::new(best_hash.d, best_nonce.to_le_bytes())
    }

    pub fn check_num_cores(&self, cores: u64) {
        let num_cores = num_cpus::get() as u64;
        if cores > num_cores {
            println!(
                "{} Cannot exceed available cores ({})",
                "WARNING".bold().yellow(),
                num_cores
            );
        }
    }

    async fn should_reset(&self, config: Config) -> bool {
        let clock = get_clock(&self.rpc_client).await;
        config
            .last_reset_at
            .saturating_add(EPOCH_DURATION)
            .saturating_sub(5)
            .le(&clock.unix_timestamp)
    }

    async fn get_cutoff(&self, proof: Proof, buffer_time: u64) -> u64 {
        let clock = get_clock(&self.rpc_client).await;
        proof
            .last_hash_at
            .saturating_add(60)
            .saturating_sub(buffer_time as i64)
            .saturating_sub(clock.unix_timestamp)
            .max(0) as u64
    }

    async fn find_bus(&self) -> Pubkey {
        if let Ok(accounts) = self.rpc_client.get_multiple_accounts(&BUS_ADDRESSES).await {
            accounts
                .iter()
                .filter_map(|account| account.as_ref().and_then(|acc| Bus::try_from_bytes(&acc.data).ok()))
                .max_by_key(|bus| bus.rewards)
                .map(|bus| BUS_ADDRESSES[bus.id as usize])
                .unwrap_or_else(|| {
                    let i = rand::thread_rng().gen_range(0..BUS_COUNT);
                    BUS_ADDRESSES[i]
                })
        } else {
            let i = rand::thread_rng().gen_range(0..BUS_COUNT);
            BUS_ADDRESSES[i]
        }
    }
}

fn calculate_multiplier(balance: u64, top_balance: u64) -> f64 {
    1.0 + (balance as f64 / top_balance as f64).min(1.0)
}

fn format_duration(seconds: u32) -> String {
    let minutes = seconds / 60;
    let remaining_seconds = seconds % 60;
    format!("{:02}:{:02}", minutes, remaining_seconds)
}
