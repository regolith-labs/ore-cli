use std::{
    str::FromStr,
    sync::{Arc, RwLock},
    time::Instant,
    usize,
};

use colored::*;
use drillx::{
    equix::{self},
    Hash, Solution,
};
use mpl_token_metadata::accounts::Metadata;
use ore_api::{
    consts::{BUS_ADDRESSES, BUS_COUNT, EPOCH_DURATION},
    state::{Bus, Config},
};
use ore_boost_api::state::{boost_pda, stake_pda};
use rand::Rng;
use solana_program::{program_pack::Pack, pubkey::Pubkey};
use solana_rpc_client::{nonblocking::rpc_client::RpcClient, spinner};
use solana_sdk::signer::Signer;
use spl_token::state::Mint;
use steel::AccountDeserialize;

use crate::{
    args::MineArgs,
    error::Error,
    pool::Pool,
    send_and_confirm::ComputeBudget,
    utils::{
        amount_u64_to_string, get_boost, get_clock, get_config, get_stake,
        get_updated_proof_with_authority, proof_pubkey,
    },
    Miner,
};

impl Miner {
    pub async fn mine(&self, args: MineArgs) -> Result<(), Error> {
        match args.pool_url {
            Some(ref pool_url) => {
                let pool = &Pool {
                    http_client: reqwest::Client::new(),
                    pool_url: pool_url.clone(),
                };
                self.mine_pool(args, pool).await?;
            }
            None => {
                self.mine_solo(args).await;
            }
        }
        Ok(())
    }

    async fn mine_solo(&self, args: MineArgs) {
        // Open account, if needed.
        let signer = self.signer();
        self.open().await;

        // Check num threads
        self.check_num_cores(args.cores);

        // Fetch boost data
        let boost_data_1 =
            fetch_boost_data(self.rpc_client.clone(), signer.pubkey(), &args.boost_1).await;
        let boost_data_2 =
            fetch_boost_data(self.rpc_client.clone(), signer.pubkey(), &args.boost_2).await;
        let boost_data_3 =
            fetch_boost_data(self.rpc_client.clone(), signer.pubkey(), &args.boost_3).await;

        // Start mining loop
        let mut last_hash_at = 0;
        let mut last_balance = 0;
        loop {
            // Fetch proof
            let config = get_config(&self.rpc_client).await;
            let proof =
                get_updated_proof_with_authority(&self.rpc_client, signer.pubkey(), last_hash_at)
                    .await;

            // Print unclaimed balance
            println!(
                "\n\nBalance: {} ORE{}",
                amount_u64_to_string(proof.balance),
                if last_hash_at.gt(&0) {
                    format!(
                        "\n  Change: {} ORE",
                        amount_u64_to_string(proof.balance.saturating_sub(last_balance))
                    )
                } else {
                    "".to_string()
                },
            );

            // Print boosts
            log_boost_data(self.rpc_client.clone(), &boost_data_1, 1).await;
            log_boost_data(self.rpc_client.clone(), &boost_data_2, 2).await;
            log_boost_data(self.rpc_client.clone(), &boost_data_3, 3).await;
            last_hash_at = proof.last_hash_at;
            last_balance = proof.balance;

            // Calculate cutoff time
            let cutoff_time = self.get_cutoff(proof.last_hash_at, args.buffer_time).await;

            // Build nonce indices
            let mut nonce_indices = Vec::with_capacity(args.cores as usize);
            for n in 0..(args.cores) {
                let nonce = u64::MAX.saturating_div(args.cores).saturating_mul(n);
                nonce_indices.push(nonce);
            }

            // Run drillx
            let solution = Self::find_hash_par(
                proof.challenge,
                cutoff_time,
                args.cores,
                config.min_difficulty as u32,
                nonce_indices.as_slice(),
            )
            .await;

            // Build instruction set
            let mut ixs = vec![ore_api::sdk::auth(proof_pubkey(signer.pubkey()))];
            let mut compute_budget = 600_000;

            // Check for reset
            if self.should_reset(config).await && rand::thread_rng().gen_range(0..100).eq(&0) {
                compute_budget += 100_000;
                ixs.push(ore_api::sdk::reset(signer.pubkey()));
            }

            // Build option (boost) accounts
            let mut optional_accounts: Vec<Pubkey> = vec![];
            optional_accounts = [optional_accounts, BoostData::to_vec(&boost_data_1)].concat();
            optional_accounts = [optional_accounts, BoostData::to_vec(&boost_data_2)].concat();
            optional_accounts = [optional_accounts, BoostData::to_vec(&boost_data_3)].concat();
            // Build mine ix
            let ix = ore_api::sdk::mine(
                signer.pubkey(),
                signer.pubkey(),
                self.find_bus().await,
                solution,
                optional_accounts,
            );
            ixs.push(ix);

            // Submit transaction
            self.send_and_confirm(&ixs, ComputeBudget::Fixed(compute_budget), false)
                .await
                .ok();
        }
    }

    async fn mine_pool(&self, args: MineArgs, pool: &Pool) -> Result<(), Error> {
        // register, if needed
        let mut pool_member = pool.post_pool_register(self).await?;
        let nonce_index = pool_member.id as u64;
        // get on-chain pool accounts
        let pool_address = pool.get_pool_address().await?;
        let mut pool_member_onchain: ore_pool_api::state::Member;
        // Check num threads
        self.check_num_cores(args.cores);
        // Start mining loop
        let mut last_hash_at = 0;
        let mut last_balance: i64;
        loop {
            // Fetch latest challenge
            let member_challenge = pool.get_updated_pool_challenge(last_hash_at).await?;
            // Increment last balance and hash
            last_balance = pool_member.total_balance;
            last_hash_at = member_challenge.challenge.lash_hash_at;
            // Compute cutoff time
            let cutoff_time = self.get_cutoff(last_hash_at, member_challenge.buffer).await;
            // Build nonce indices
            let num_total_members = member_challenge.num_total_members.max(1);
            let u64_unit = u64::MAX.saturating_div(num_total_members);
            let left_bound = u64_unit.saturating_mul(nonce_index);
            let range_per_core = u64_unit.saturating_div(args.cores);
            let mut nonce_indices = Vec::with_capacity(args.cores as usize);
            for n in 0..(args.cores) {
                let index = left_bound + n * range_per_core;
                nonce_indices.push(index);
            }
            // Run drillx
            let solution = Self::find_hash_par(
                member_challenge.challenge.challenge,
                cutoff_time,
                args.cores,
                member_challenge.challenge.min_difficulty as u32,
                nonce_indices.as_slice(),
            )
            .await;
            // Post solution to operator
            pool.post_pool_solution(self, &solution).await?;
            // Get updated pool member
            pool_member = pool.get_pool_member(self).await?;
            // Get updated on-chain pool member
            pool_member_onchain = pool
                .get_pool_member_onchain(self, pool_address.address)
                .await?;
            // Print progress
            println!(
                "Claimable ORE balance: {}",
                amount_u64_to_string(pool_member_onchain.balance)
            );
            if last_hash_at.gt(&0) {
                println!(
                    "Change of ORE credits in pool: {}",
                    amount_u64_to_string(
                        pool_member.total_balance.saturating_sub(last_balance) as u64
                    )
                )
            }
        }
    }

    async fn find_hash_par(
        challenge: [u8; 32],
        cutoff_time: u64,
        cores: u64,
        min_difficulty: u32,
        nonce_indices: &[u64],
    ) -> Solution {
        // Dispatch job to each thread
        let progress_bar = Arc::new(spinner::new_progress_bar());
        let global_best_difficulty = Arc::new(RwLock::new(0u32));
        progress_bar.set_message("Mining...");
        let core_ids = core_affinity::get_core_ids().unwrap();
        let core_ids = core_ids.into_iter().filter(|id| id.id < (cores as usize));
        let handles: Vec<_> = core_ids
            .map(|i| {
                let global_best_difficulty = Arc::clone(&global_best_difficulty);
                std::thread::spawn({
                    let progress_bar = progress_bar.clone();
                    let nonce = nonce_indices[i.id];
                    let mut memory = equix::SolverMemory::new();
                    move || {
                        // Pin to core
                        let _ = core_affinity::set_for_current(i);

                        // Start hashing
                        let timer = Instant::now();
                        let mut nonce = nonce;
                        let mut best_nonce = nonce;
                        let mut best_difficulty = 0;
                        let mut best_hash = Hash::default();
                        loop {
                            // Get hashes
                            let hxs = drillx::hashes_with_memory(
                                &mut memory,
                                &challenge,
                                &nonce.to_le_bytes(),
                            );

                            // Look for best difficulty score in all hashes
                            for hx in hxs {
                                let difficulty = hx.difficulty();
                                if difficulty.gt(&best_difficulty) {
                                    best_nonce = nonce;
                                    best_difficulty = difficulty;
                                    best_hash = hx;
                                    if best_difficulty.gt(&*global_best_difficulty.read().unwrap())
                                    {
                                        *global_best_difficulty.write().unwrap() = best_difficulty;
                                    }
                                }
                            }

                            // Exit if time has elapsed
                            if nonce % 100 == 0 {
                                let global_best_difficulty =
                                    *global_best_difficulty.read().unwrap();
                                if timer.elapsed().as_secs().ge(&cutoff_time) {
                                    if i.id == 0 {
                                        progress_bar.set_message(format!(
                                            "Mining... (difficulty {})",
                                            global_best_difficulty,
                                        ));
                                    }
                                    if global_best_difficulty.ge(&min_difficulty) {
                                        // Mine until min difficulty has been met
                                        break;
                                    }
                                } else if i.id == 0 {
                                    progress_bar.set_message(format!(
                                        "Mining... (difficulty {}, time {})",
                                        global_best_difficulty,
                                        format_duration(
                                            cutoff_time.saturating_sub(timer.elapsed().as_secs())
                                                as u32
                                        ),
                                    ));
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
            "Best hash: {} (difficulty {})",
            bs58::encode(best_hash.h).into_string(),
            best_difficulty
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
            .saturating_sub(5) // Buffer
            .le(&clock.unix_timestamp)
    }

    async fn get_cutoff(&self, last_hash_at: i64, buffer_time: u64) -> u64 {
        let clock = get_clock(&self.rpc_client).await;
        last_hash_at
            .saturating_add(60)
            .saturating_sub(buffer_time as i64)
            .saturating_sub(clock.unix_timestamp)
            .max(0) as u64
    }

    async fn find_bus(&self) -> Pubkey {
        // Fetch the bus with the largest balance
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

        // Otherwise return a random bus
        let i = rand::thread_rng().gen_range(0..BUS_COUNT);
        BUS_ADDRESSES[i]
    }
}

#[derive(Clone)]
struct BoostData {
    boost_address: Pubkey,
    stake_address: Pubkey,
    mint: Mint,
    metadata: Option<Metadata>,
}

impl BoostData {
    fn to_vec(boost_data: &Option<BoostData>) -> Vec<Pubkey> {
        match boost_data {
            Some(boost_data) => {
                vec![boost_data.boost_address, boost_data.stake_address]
            }
            None => vec![],
        }
    }
}

async fn fetch_boost_data(
    rpc: Arc<RpcClient>,
    authority: Pubkey,
    mint_address: &Option<String>,
) -> Option<BoostData> {
    let Some(mint_address) = mint_address else {
        return None;
    };
    let mint_address = Pubkey::from_str(&mint_address).unwrap();
    let boost_address = boost_pda(mint_address).0;
    let stake_address = stake_pda(authority, boost_address).0;
    let mint = rpc
        .get_account_data(&mint_address)
        .await
        .map(|data| Mint::unpack(&data).unwrap())
        .unwrap();
    let metadata = rpc
        .get_account_data(&Metadata::find_pda(&mint_address).0)
        .await
        .ok()
        .map(|data| Metadata::from_bytes(&data).unwrap());
    Some(BoostData {
        boost_address,
        stake_address,
        mint,
        metadata,
    })
}

async fn log_boost_data(rpc: Arc<RpcClient>, boost_data: &Option<BoostData>, id: u64) {
    if let Some(boost_data) = boost_data {
        let boost = get_boost(&rpc, boost_data.boost_address).await;
        let stake = get_stake(&rpc, boost_data.stake_address).await;
        let multiplier =
            (boost.multiplier as f64) * (stake.balance as f64) / (boost.total_stake as f64);
        println!(
            "  Boost {}: {:12}x ({})",
            id,
            multiplier,
            format!(
                "{} of {}{}",
                stake.balance as f64 / 10f64.powf(boost_data.mint.decimals as f64),
                boost.total_stake as f64 / 10f64.powf(boost_data.mint.decimals as f64),
                boost_data
                    .clone()
                    .metadata
                    .map_or("".to_string(), |m| format!(" {}", m.symbol))
            )
        );
    }
}

fn format_duration(seconds: u32) -> String {
    let minutes = seconds / 60;
    let remaining_seconds = seconds % 60;
    format!("{:02}:{:02}", minutes, remaining_seconds)
}
