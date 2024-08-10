use std::{sync::Arc, sync::atomic::{AtomicU64, AtomicU32, Ordering}, time::Instant};
use std::thread;
use std::sync::mpsc::channel;

use colored::*;
use drillx::{
    equix::{self},
    Hash, Solution,
};
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
            let proof =
                get_updated_proof_with_authority(&self.rpc_client, signer.pubkey(), last_hash_at)
                    .await;
            println!(
                "\n\nStake: {} ORE\n{}  Multiplier: {:12}x",
                amount_u64_to_string(proof.balance),
                if last_hash_at.gt(&0) {
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

            let solution =
                Self::find_hash_par(proof, cutoff_time, args.cores, config.min_difficulty as u32)
                    .await;

            let mut ixs = vec![ore_api::instruction::auth(proof_pubkey(signer.pubkey()))];
            let mut compute_budget = 500_000;
            if self.should_reset(config).await && rand::thread_rng().gen_range(0..100).eq(&0) {
                compute_budget += 100_000;
                ixs.push(ore_api::instruction::reset(signer.pubkey()));
            }

            ixs.push(ore_api::instruction::mine(
                signer.pubkey(),
                signer.pubkey(),
                self.find_bus().await,
                solution,
            ));

            self.send_and_confirm(&ixs, ComputeBudget::Fixed(compute_budget), false)
                .await
                .ok();
        }
    }

    async fn find_hash_par(
        proof: Proof,
        mut cutoff_time: u64,  // Make cutoff_time mutable
        cores: u64,
        min_difficulty: u32,
    ) -> Solution {
        // Create a thread pool and use a channel for load balancing
        let (tx, rx) = channel();
        let progress_bar = Arc::new(spinner::new_progress_bar());
        let global_best_difficulty = Arc::new(AtomicU32::new(0));
        let global_total_hashes = Arc::new(AtomicU64::new(0));
        progress_bar.set_message("Mining...");

        let start_time = Instant::now();

        for i in 0..cores {
            let tx = tx.clone();
            let global_best_difficulty = Arc::clone(&global_best_difficulty);
            let global_total_hashes = Arc::clone(&global_total_hashes);
            let proof = proof.clone();
            let progress_bar = progress_bar.clone();

            thread::spawn(move || {
                let mut memory = equix::SolverMemory::new();
                let timer = Instant::now();
                let mut nonce = u64::MAX.saturating_div(cores).saturating_mul(i);
                let mut best_nonce = nonce;
                let mut best_difficulty = 0;
                let mut best_hash = Hash::default();
                loop {
                    if let Ok(hx) = drillx::hash_with_memory(
                        &mut memory,
                        &proof.challenge,
                        &nonce.to_le_bytes(),
                    ) {
                        let difficulty = hx.difficulty();
                        if difficulty.gt(&best_difficulty) {
                            best_nonce = nonce;
                            best_difficulty = difficulty;
                            best_hash = hx;

                            // Update global best difficulty
                            let prev_best_difficulty = global_best_difficulty.fetch_max(best_difficulty, Ordering::Relaxed);
                            
                            // Extend cutoff time if a new higher difficulty hash is found
                            if best_difficulty > prev_best_difficulty {
                                cutoff_time += 00;
                            }
                        }
                    }

                    global_total_hashes.fetch_add(1, Ordering::Relaxed);

                    if nonce % 100 == 0 {
                        let global_best_difficulty = global_best_difficulty.load(Ordering::Relaxed);
                        let total_hashes = global_total_hashes.load(Ordering::Relaxed);
                        let elapsed_time = start_time.elapsed().as_secs_f64();
                        let hash_rate = total_hashes as f64 / elapsed_time;

                        if timer.elapsed().as_secs().ge(&cutoff_time) {
                            if i == 0 {
                                progress_bar.set_message(format!(
                                    "Mining... (difficulty {}, time {}, {:.2} H/s)",
                                    global_best_difficulty,
                                    format_duration(
                                        cutoff_time
                                            .saturating_sub(timer.elapsed().as_secs())
                                            as u32
                                    ),
                                    hash_rate,
                                ));
                            }
                            if global_best_difficulty.ge(&min_difficulty) {
                                break;
                            }
                        } else if i == 0 {
                            progress_bar.set_message(format!(
                                "Mining... (difficulty {}, time {}, {:.2} H/s)",
                                global_best_difficulty,
                                format_duration(
                                    cutoff_time.saturating_sub(timer.elapsed().as_secs())
                                        as u32
                                ),
                                hash_rate,
                            ));
                        }
                    }

                    nonce += 1;
                }

                // Send the result back to the main thread
                tx.send((best_nonce, best_difficulty, best_hash)).unwrap();
            });
        }

        // Collect results from threads
        let mut best_nonce = 0;
        let mut best_difficulty = 0;
        let mut best_hash = Hash::default();

        for _ in 0..cores {
            let (nonce, difficulty, hash) = rx.recv().unwrap();
            if difficulty > best_difficulty {
                best_difficulty = difficulty;
                best_nonce = nonce;
                best_hash = hash;
            }
        }

        let total_hashes = global_total_hashes.load(Ordering::Relaxed);
        let elapsed_time = start_time.elapsed().as_secs_f64();
        let hash_rate = total_hashes as f64 / elapsed_time;

        progress_bar.finish_with_message(format!(
            "Best hash: {} (difficulty {}, {:.2} H/s)",
            bs58::encode(best_hash.h).into_string(),
            best_difficulty,
            hash_rate,
        ));

        Solution::new(best_hash.d, best_nonce.to_le_bytes())
    }

    pub fn check_num_cores(&self, cores: u64) {
        let num_cores = num_cpus::get() as u64;
        if cores.gt(&num_cores) {
            println!(
                "{} Cannot exceeds available cores ({})",
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
            let mut top_bus_balance: u64 = 0;
            let mut top_bus = BUS_ADDRESSES[0];
            for account in accounts {
                if let Some(account) = account {
                    if let Ok(bus) = Bus::try_from_bytes(&account.data) {
                        if bus.rewards.gt(&top_bus_balance) {
                            top_bus_balance = bus.rewards;
                            top_bus = BUS_ADDRESSES[bus.id as usize];
                        }
                    }
                }
            }
            return top_bus;
        }

        let i = rand::thread_rng().gen_range(0..BUS_COUNT);
        BUS_ADDRESSES[i]
    }
}

fn calculate_multiplier(balance: u64, top_balance: u64) -> f64 {
    1.0 + (balance as f64 / top_balance as f64).min(1.0f64)
}

fn format_duration(seconds: u32) -> String {
    let minutes = seconds / 60;
    let remaining_seconds = seconds % 60;
    format!("{:02}:{:02}", minutes, remaining_seconds)
}
