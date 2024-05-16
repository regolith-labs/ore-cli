use std::sync::Arc;
use std::time::Instant;

use colored::*;
use drillx::{
    equix::{self},
    Hash, Solution,
};
use ore::{self, state::Proof, BUS_ADDRESSES, BUS_COUNT, EPOCH_DURATION};
use rand::Rng;
use solana_program::pubkey::Pubkey;
use solana_rpc_client::spinner;
use solana_sdk::signer::Signer;
use chrono::prelude::*;

use crate::{
    args::MineArgs,
    send_and_confirm::ComputeBudget,
    utils::{amount_u64_to_string, get_clock, get_config, get_proof},
    Miner,
};

impl Miner {
    pub async fn mine(&self, args: MineArgs) {
        // Register, if needed.
        let signer = self.signer();
        self.register().await;

        // Check num threads
        self.check_num_cores(args.threads);
		let mut pass=1;

        // Start mining loop
        loop {
			let pass_start_time = Instant::now();
			println!("\nPass {} started at {}",
				pass,
				Utc::now().format("%H:%M:%S on %Y-%m-%d").to_string(),
			);

            // Fetch proof
            let proof = get_proof(&self.rpc_client, signer.pubkey()).await;

			// Calc cutoff time
            let cutoff_time = self.get_cutoff(proof, args.buffer_time).await;

			println!("        Currently staked ORE: {}\tMining duration: {}s",
                amount_u64_to_string(proof.balance),
				cutoff_time,
            );


            // Run drillx
			// let hash_start_time = Instant::now();
			let solution = self.find_hash_par(proof, cutoff_time, args.threads).await;
			// let hash_duration = hash_start_time.elapsed();

            // Submit most difficult hash
            let mut ixs = vec![];
            if self.needs_reset().await {
                ixs.push(ore::instruction::reset(signer.pubkey()));
            }
            ixs.push(ore::instruction::mine(
                signer.pubkey(),
                find_bus(),
                solution,
            ));
			// let submit_start_time = Instant::now();
            self.send_and_confirm(&ixs, ComputeBudget::Fixed(500_000), false)
                .await
                .ok();
			// let submit_duration = submit_start_time.elapsed();

			println!("  [{}s] Completed Pass {}",
				pass_start_time.elapsed().as_secs().to_string(),
				pass,
			);
			pass+=1;
        }
    }

    async fn find_hash_par(&self, proof: Proof, cutoff_time: u64, threads: u64) -> Solution {
        // Dispatch job to each thread
		let timer = Instant::now();
		let progress_bar = Arc::new(spinner::new_progress_bar());
        progress_bar.set_message(format!("[{}s to go] Mining...", cutoff_time));
		let handles: Vec<_> = (0..threads)
            .map(|i| {
                std::thread::spawn({
                    let proof = proof.clone();
                    let progress_bar = progress_bar.clone();
                    let mut memory = equix::SolverMemory::new();
                    move || {
                        let mut nonce = u64::MAX.saturating_div(threads).saturating_mul(i);
                        let mut best_nonce = nonce;
                        let mut best_difficulty = 0;
                        let mut best_hash = Hash::default();
						let mut last_elapsed:u64 = 0;
                        loop {
		                    // Create hash
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
                                }
                            }

                            // Exit if time has elapsed
                            if nonce % 100 == 0 {
                                if timer.elapsed().as_secs().ge(&cutoff_time) {
                                    if best_difficulty.gt(&ore::MIN_DIFFICULTY) {
                                        // Mine until min difficulty has been met
                                        break;
                                    }
                                } else if i == 0 {
									let next_elapsed=timer.elapsed().as_secs();
									if next_elapsed != last_elapsed {
										progress_bar.set_message(format!(
											"[{}s to go] Mining... Difficulty so far: {}",
											cutoff_time.saturating_sub(next_elapsed),
											best_difficulty,
										));
										last_elapsed=next_elapsed;
									}
                                }
                            }

                            // Increment nonce
                            nonce += 1;
                        }

                        // Return the best nonce
                        (best_nonce, best_difficulty, best_hash)
                    }
                })
            })
            .collect();

        // Join handles and return best nonce
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

        // Update log
		progress_bar.finish_with_message(format!(
            "[{}s] Best Difficulty: {} Hash: {} ",
			timer.elapsed().as_secs().to_string(),
            best_difficulty.to_string().bold().yellow(),
            bs58::encode(best_hash.h).into_string().dimmed(),
        ));

        Solution::new(best_hash.d, best_nonce.to_le_bytes())
    }

    pub fn check_num_cores(&self, threads: u64) {
        // Check num threads
        let num_cores = num_cpus::get() as u64;
        if threads.gt(&num_cores) {
            println!(
                "{} Number of threads ({}) exceeds available cores ({})",
                "WARNING".bold().yellow(),
                threads,
                num_cores
            );
        }
    }

    async fn needs_reset(&self) -> bool {
        let clock = get_clock(&self.rpc_client).await;
        let config = get_config(&self.rpc_client).await;
        config
            .last_reset_at
            .saturating_add(EPOCH_DURATION)
            .saturating_sub(5) // Buffer
            .le(&clock.unix_timestamp)
    }

    async fn get_cutoff(&self, proof: Proof, buffer_time: u64) -> u64 {
        let clock = get_clock(&self.rpc_client).await;
        let mut retval=proof.last_hash_at
            .saturating_add(60)
            .saturating_sub(buffer_time as i64)
            .saturating_sub(clock.unix_timestamp)
            .max(0) as u64;
		if retval==0 {
			retval=(60 as i64).saturating_sub(buffer_time as i64).max(0) as u64;
		}
		return retval;
    }
}

// TODO Pick a better strategy (avoid draining bus)
fn find_bus() -> Pubkey {
    let i = rand::thread_rng().gen_range(0..BUS_COUNT);
    BUS_ADDRESSES[i]
}
