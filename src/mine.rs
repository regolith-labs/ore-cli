use std::{
    sync::{Arc, RwLock},
    time::{Instant, Duration},
    usize, io::stdout, thread::sleep,
};

use b64::FromBase64;
use colored::*;
use crossterm::{execute, terminal::{Clear, ClearType}, cursor::MoveTo};
use drillx::{
    equix::{self},
    Hash, Solution,
};
use ore_api::{
    consts::{BUS_ADDRESSES, BUS_COUNT, EPOCH_DURATION},
    state::{Bus, Config, proof_pda}, event::MineEvent,
};
use ore_boost_api::state::reservation_pda;
use rand::Rng;
use solana_program::pubkey::Pubkey;
use solana_rpc_client::spinner;
use solana_sdk::{signer::Signer, signature::Signature};
use solana_transaction_status::{UiTransactionEncoding, option_serializer::OptionSerializer};
use steel::AccountDeserialize;
use tabled::{Table, Tabled, settings::{Style, Color, object::{Columns, Rows}, Alignment, Highlight, style::BorderColor, Border}};

use crate::{
    args::MineArgs,
    error::Error,
    pool::Pool,
    send_and_confirm::ComputeBudget,
    utils::{
        amount_u64_to_string, get_clock, get_config,
        get_updated_proof_with_authority, proof_pubkey, get_reservation, format_timestamp,
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
        self.open().await;

        // Check num threads
        self.check_num_cores(args.cores);

        // Generate addresses
        let signer = self.signer();
        let proof_address = proof_pda(signer.pubkey()).0;
        let reservation_address = reservation_pda(proof_address).0;

        // Start mining loop
        let mut last_hash_at = 0;
        loop {            
            // Fetch accounts
            let config = get_config(&self.rpc_client).await;
            let proof = get_updated_proof_with_authority(
                &self.rpc_client, 
                signer.pubkey(), 
                last_hash_at
            ).await.unwrap();
            let reservation = get_reservation(&self.rpc_client, reservation_address).await;

            // Log mining table
            self.update_solo_mining_table();

            // Track timestamp
            last_hash_at = proof.last_hash_at;

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
                None,
            )
            .await;

            // Build instruction set
            let mut ixs = vec![ore_api::sdk::auth(proof_pubkey(signer.pubkey()))];
            let mut compute_budget = 750_000;

            // Check for reset
            if self.should_reset(config).await
            // && rand::thread_rng().gen_range(0..100).eq(&0)
            {
                compute_budget += 100_000;
                ixs.push(ore_api::sdk::reset(signer.pubkey()));
            }

            // Build mine ix
            let boost_address = reservation
                .map(|r| if r.boost == Pubkey::default() {
                    None
                } else {
                    Some(r.boost)
                })
                .unwrap_or(None);
            let boost_keys = if let Some(boost_address) = boost_address {
                Some((boost_address, reservation_address))
            } else {
                None
            };
            let mine_ix = ore_api::sdk::mine(
                signer.pubkey(),
                signer.pubkey(),
                self.find_bus().await,
                solution,
                boost_keys,
            );
            ixs.push(mine_ix);

            // Build rotation ix
            let rotate_ix = ore_boost_api::sdk::rotate(signer.pubkey(), proof_address);
            ixs.push(rotate_ix);

            // Submit transaction
            match self.send_and_confirm(&ixs, ComputeBudget::Fixed(compute_budget), false).await {
                Ok(sig) => {
                    self.fetch_solo_mine_event(sig).await
                },
                Err(err) => {
                    let mining_data = SoloMiningData::failed();
                    let mut data = self.solo_mining_data.write().unwrap();
                    data.remove(0);
                    data.insert(0, mining_data); 
                    drop(data);

                    // Log mining table
                    self.update_solo_mining_table();
                    println!("{}: {}", "ERROR".bold().red(), err);

                    return;
                }
            }
        }
    }

    async fn mine_pool(&self, args: MineArgs, pool: &Pool) -> Result<(), Error> {
        // Register, if needed
        let pool_member = pool.post_pool_register(self).await?;
        let nonce_index = pool_member.id as u64;

        // Check num threads
        self.check_num_cores(args.cores);

        // Init channel for continuous submission
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<Solution>();
        tokio::spawn({
            let miner = self.clone();
            let pool = pool.clone();
            async move {
                while let Some(solution) = rx.recv().await {
                    if let Err(err) = pool.post_pool_solution(&miner, &solution).await {
                        println!("error submitting solution: {:?}", err);
                    }
                }
            }
        });

        // Start mining loop
        let mut last_hash_at = 0;
        loop {
            // Fetch latest challenge
            let member_challenge = match pool.get_updated_pool_challenge(self, last_hash_at).await {
                Err(_err) => {
                    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
                    continue;
                }
                Ok(member_challenge) => member_challenge,
            };

            // Log mining table
            self.update_pool_mining_table();

            // Increment last balance and hash
            last_hash_at = member_challenge.challenge.lash_hash_at;

            // Compute cutoff time
            let cutoff_time = self.get_cutoff(last_hash_at, args.buffer_time).await;

            // Build nonce indices
            let num_total_members = member_challenge.num_total_members.max(1);
            let member_search_space_size = u64::MAX.saturating_div(num_total_members);
            let device_search_space_size = member_search_space_size.saturating_div(member_challenge.num_devices as u64);

            // Calculate bounds on 
            if member_challenge.device_id.gt(&member_challenge.num_devices) {
                return Err(Error::TooManyDevices);
            }
            let device_id = member_challenge.device_id.saturating_sub(1) as u64;
            let left_bound =
                member_search_space_size.saturating_mul(nonce_index) + device_id.saturating_mul(device_search_space_size);

            // Split nonce-device space for muliple cores
            let range_per_core = device_search_space_size.saturating_div(args.cores);
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
                Some(tx.clone()),
            )
            .await;

            // Post solution to pool server
            match pool.post_pool_solution(self, &solution).await {
                Err(_err) => {
                    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
                    continue;
                }
                Ok(()) => {
                    self.fetch_pool_mine_event(pool, last_hash_at).await;
                }
            }
        }
    }

    async fn find_hash_par(
        challenge: [u8; 32],
        cutoff_time: u64,
        cores: u64,
        min_difficulty: u32,
        nonce_indices: &[u64],
        pool_channel: Option<tokio::sync::mpsc::UnboundedSender<Solution>>,
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
                    let pool_channel = pool_channel.clone();
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
                                        // Update best global difficulty
                                        *global_best_difficulty.write().unwrap() = best_difficulty;

                                        // Continuously upload best solution to pool
                                        if difficulty.ge(&min_difficulty) {
                                            if let Some(ref ch) = pool_channel {
                                                let digest = best_hash.d;
                                                let nonce = nonce.to_le_bytes();
                                                let solution = Solution {
                                                    d: digest,
                                                    n: nonce,
                                                };
                                                if let Err(err) = ch.send(solution) {
                                                    println!("{} {:?}", "ERROR".bold().red(), err);
                                                }
                                            }
                                        }
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
                                            "Mining...\n  Best score: {}",
                                            global_best_difficulty,
                                        ));
                                    }
                                    if global_best_difficulty.ge(&min_difficulty) {
                                        // Mine until min difficulty has been met
                                        break;
                                    }
                                } else if i.id == 0 {
                                    progress_bar.set_message(format!(
                                        "Mining...\n  Best score: {}\n  Time remaining: {}",
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
        let mut best_nonce: u64 = 0;
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

    async fn fetch_solo_mine_event(&self, sig: Signature) {        
        // Add loading row
        let mining_data = SoloMiningData::fetching(sig);
        let mut data = self.solo_mining_data.write().unwrap();
        data.insert(0, mining_data); 
        if data.len() >= 12 {
            data.pop();
        }
        drop(data);

        // Update table
        self.update_solo_mining_table();
        
        // Poll for transaction
        let mut tx;
        let mut attempts = 0;
        loop {
            tx = self.rpc_client.get_transaction(&sig, UiTransactionEncoding::Json).await;
            if tx.is_ok() {
                break;
            }
            sleep(Duration::from_secs(1));
            attempts += 1;
            if attempts > 30 {
                break;
            }
        }

        // Parse transaction response
        if let Ok(tx) = tx {
            if let Some(meta) = tx.transaction.meta {
                if let OptionSerializer::Some(log_messages) = meta.log_messages {
                    if let Some(return_log) = log_messages.iter().find(|log| log.starts_with("Program return: ")) {
                        if let Some(return_data) = return_log.strip_prefix(&format!("Program return: {} ", ore_api::ID)) {
                            if let Ok(return_data) = return_data.from_base64() {
                                let mut data = self.solo_mining_data.write().unwrap();
                                let event = MineEvent::from_bytes(&return_data);
                                let mining_data = SoloMiningData {
                                    signature: sig.to_string(),
                                    block: tx.slot.to_string(),
                                    timestamp: format_timestamp(tx.block_time.unwrap_or_default()),
                                    difficulty: event.difficulty.to_string(),
                                    base_reward: amount_u64_to_string(event.net_base_reward),
                                    boost_reward: amount_u64_to_string(event.net_miner_boost_reward),
                                    total_reward: amount_u64_to_string(event.net_reward),
                                    timing: format!("{}s", event.timing),
                                    status: "Confirmed".bold().green().to_string(),
                                };
                                data.remove(0);
                                data.insert(0, mining_data);
                            }
                        }
                    }
                }
            }
        }
    }

    async fn fetch_pool_mine_event(&self, pool: &Pool, last_hash_at: i64) {
        if let Ok(event) = pool.get_latest_pool_event(self.signer().pubkey(), last_hash_at).await {
            // Add loading row
            let mining_data = PoolMiningData {
                signature: event.signature.to_string(),
                block: event.block.to_string(),
                timestamp: format_timestamp(event.timestamp as i64),
                timing: format!("{}s", event.timing),
                difficulty: event.difficulty.to_string(),
                base_reward: amount_u64_to_string(event.net_base_reward),
                boost_reward: amount_u64_to_string(event.net_miner_boost_reward),
                total_reward: amount_u64_to_string(event.net_reward),
                my_difficulty: event.member_difficulty.to_string(),
                my_reward: amount_u64_to_string(event.member_reward).bold().yellow().to_string(),
            };

            // Add loading row
            let mut data = self.pool_mining_data.write().unwrap();
            data.insert(0, mining_data); 
            if data.len() >= 12 {
                data.pop();
            }
            drop(data);
        }
    }

    fn update_solo_mining_table(&self) {
        execute!(stdout(), Clear(ClearType::All), MoveTo(0, 0)).unwrap();
        let mut rows: Vec<SoloMiningData>  = vec![];
        let data = self.solo_mining_data.read().unwrap();
        rows.extend(data.iter().cloned());
        let mut table = Table::new(&rows);
        table.with(Style::blank());
        table.modify(Columns::new(1..), Alignment::right());
        table.modify(Rows::first(), Color::BOLD);
        table.with(Highlight::new(Rows::single(1)).color(BorderColor::default().top(Color::FG_WHITE)));
        table.with(Highlight::new(Rows::single(1)).border(Border::new().top('━')));
        println!("\n{}\n", table);
    }

    fn update_pool_mining_table(&self) {
        execute!(stdout(), Clear(ClearType::All), MoveTo(0, 0)).unwrap();
        let mut rows: Vec<PoolMiningData>  = vec![];
        let data = self.pool_mining_data.read().unwrap();
        rows.extend(data.iter().cloned());
        let mut table = Table::new(&rows);
        table.with(Style::blank());
        table.modify(Columns::new(1..), Alignment::right());
        table.modify(Rows::first(), Color::BOLD);
        table.with(Highlight::new(Rows::single(1)).color(BorderColor::default().top(Color::FG_WHITE)));
        table.with(Highlight::new(Rows::single(1)).border(Border::new().top('━')));
        println!("\n{}\n", table);
    }
}

fn format_duration(seconds: u32) -> String {
    let minutes = seconds / 60;
    let remaining_seconds = seconds % 60;
    format!("{:02}:{:02}", minutes, remaining_seconds)
}

#[derive(Clone, Tabled)]
pub struct SoloMiningData {
    #[tabled(rename = "Signature")]
    signature: String,
    #[tabled(rename = "Block")]
    block: String,
    #[tabled(rename = "Timestamp")]
    timestamp: String,
    #[tabled(rename = "Timing")]
    timing: String,
    #[tabled(rename = "Score")]
    difficulty: String,
    #[tabled(rename = "Base Reward")]
    base_reward: String,
    #[tabled(rename = "Boost Reward")]
    boost_reward: String,
    #[tabled(rename = "Total Reward")]
    total_reward: String,
    #[tabled(rename = "Status")]
    status: String,
}

impl SoloMiningData {
    fn fetching(sig: Signature) -> Self {
        Self {
            signature: sig.to_string(),
            block: "–".to_string(),
            timestamp: "–".to_string(),
            difficulty: "–".to_string(),
            base_reward: "–".to_string(),
            boost_reward: "–".to_string(),
            total_reward: "–".to_string(),
            timing: "–".to_string(),
            status: "Fetching".to_string(),
        }
    }

    fn failed() -> Self {
        Self {
            signature: "–".to_string(),
            block: "–".to_string(),
            timestamp: "–".to_string(),
            difficulty: "–".to_string(),
            base_reward: "–".to_string(),
            boost_reward: "–".to_string(),
            total_reward: "–".to_string(),
            timing: "–".to_string(),
            status: "Failed".bold().red().to_string(),
        }
    }
}


#[derive(Clone, Tabled)]
pub struct PoolMiningData {
    #[tabled(rename = "Signature")]
    signature: String,
    #[tabled(rename = "Block")]
    block: String,
    #[tabled(rename = "Timestamp")]
    timestamp: String,
    #[tabled(rename = "Timing")]
    timing: String,
    #[tabled(rename = "Score")]
    difficulty: String,
    #[tabled(rename = "Pool Base Reward")]
    base_reward: String,
    #[tabled(rename = "Pool Boost Reward")]
    boost_reward: String,
    #[tabled(rename = "Pool Total Reward")]
    total_reward: String,
    #[tabled(rename = "My Score")]
    my_difficulty: String,
    #[tabled(rename = "My Reward")]
    my_reward: String,
}