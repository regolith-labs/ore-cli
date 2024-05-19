use std::fs::File;
use std::io::{BufRead, BufReader, Result};
use std::sync::Arc;
use std::collections::BTreeMap;
use std::time::{Instant, Duration};
use humantime::format_duration;
use systemstat::{System, Platform};
use chrono::prelude::*;

use colored::*;
use drillx::{
    equix::{self},
    Hash, Solution
};
use ore::{self, state::Proof, BUS_ADDRESSES, BUS_COUNT, EPOCH_DURATION, ONE_DAY};

use rand::Rng;
use solana_program::{
	pubkey::Pubkey,
    native_token::{lamports_to_sol, sol_to_lamports},
};

use solana_rpc_client::spinner;
use solana_sdk::signer::Signer;
use solana_sdk::clock::Clock;

use crate::{
    args::MineArgs,
	send_and_confirm::ComputeBudget,
    utils::{amount_u64_to_f64, get_clock, get_config, get_proof},
    Miner,
};

impl Miner {
    pub async fn mine(&self, args: MineArgs) {
		const MIN_SOL_BALANCE: f64 = 0.005;

		// Register, if needed.
        let signer = self.signer();
        self.register().await;

		let sys = System::new();

        // Check num threads
        self.check_num_cores(args.threads);

		let mut cutoff_time: u64;						// How long each pass will mine for
		let mining_start_time = Instant::now();	// When the miner was initially started
		let mut pass=1;								// This represents how many times the miner has tried to mine
		let mut current_sol_balance: f64;					// The amount of SOL in the wallet
		let mut current_staked_balance: f64;				// The amount of staked ORE in the wallet
		let mut last_sol_balance: f64 = 0.0;				// The amount of SOL in the wallet in the previous mining pass
		let mut last_staked_balance: f64 = 0.0;				// The amount of ORE in the wallet in the previous mining pass
		let mut last_pass_difficulty: u32= 0;				// The best difficulty solved in the last pass
		let mut session_ore_mined: f64 = 0.0;				// A running tally of the ORE mined in all passes (session)
		let mut session_sol_used: f64 = 0.0;				// A running tally of the SOL spent in all passes (session)
		let mut difficulties_solved: BTreeMap<u32, usize> = BTreeMap::new();	// An array that counts how many of each difficulty has been solved in this session
		let mut max_reward: f64 = 0.0;						// What has been the highest reward mined in this session
		let mut max_reward_text: String = "".to_string();	// A text string detailing the maximum reward pass

		let mut _current_ore_price:f64=self.load_ore_price();
		let mut _current_sol_price:f64=self.load_sol_price();

        // Start mining loop
        loop {
			let pass_start_time = Instant::now();

			// Download the SOL prices from coinmarketcap to display prices in $
			// self.download_sol_price();

            // Fetch proof
            let proof = get_proof(&self.rpc_client, signer.pubkey()).await;

			// Determine Wallet ORE & SOL Balances
			current_sol_balance=self.get_sol_balance(false).await;
			current_staked_balance=amount_u64_to_f64(proof.balance);

			// Calc cutoff time
			let clock = get_clock(&self.rpc_client).await;
           	cutoff_time = self.get_cutoff(proof, args.buffer_time, &clock).await;

			// Determine if Staked ORE can be withdrawing without penalty or if ORE will be burned
			let t = proof.last_claim_at.saturating_add(ONE_DAY);
			let mut claim_text="No Withdrawal Penalty".green().to_string();
			if clock.unix_timestamp.lt(&t) {	// Clock is reused from above
				let mins_to_go = t.saturating_sub(clock.unix_timestamp).saturating_div(60);
				claim_text = format!("{} {} {}",
						"Withdrawal Penalty for".bold().red(),
						mins_to_go.to_string().bold().red(),
						"mins".bold().red(),
					);
			}

			// Summarize the results of the previous mining pass
			if pass>1 {
				// Add the difference in staked ore from the previous pass to the session_ore_mined tally
				let mut last_pass_ore_mined=current_staked_balance-last_staked_balance;
				// If ore has been unstaked, then this value will be wrong for last pass so ignore it
				if last_pass_ore_mined<0.0 {
					last_pass_ore_mined=0.0;
				}
				// Not sure how to detect is additional ore has been staked
				// possible to check with proof last claimed > last pass start time?
				session_ore_mined+=last_pass_ore_mined;	// Update the session ore mined tally

				// Log if this pass is your maximum reward for this session
				if last_pass_ore_mined>max_reward {
					max_reward = last_pass_ore_mined;
					max_reward_text = format!("Max session reward: {:.11} ORE (${:.4}) at difficulty {} during pass {}.",
						last_pass_ore_mined,
						last_pass_ore_mined * _current_ore_price,
						last_pass_difficulty,
						pass,
					);
				}

				// Add the difference in sol from the previous pass to the session_sol_used tally
				let mut last_pass_sol_used=current_sol_balance-last_sol_balance;
				// Sol has been added to wallet so disregard the last passed sol_used as it is incorrect
				if last_pass_sol_used>0.0 {
					last_pass_sol_used=-0.0;
				}
				// not sure how to detect a change in sol level after the start of the last pass that is not just a transaction fee.
				session_sol_used-=last_pass_sol_used;	// Update the session sol used tally

				println!("    - Mined: {:.11}\t      Cost: {:.9}\tSession: {:.11} ORE\t{:.9} SOL",
					last_pass_ore_mined,
					last_pass_sol_used,
					session_ore_mined,
					session_sol_used,
				);

				// Show a summary of the difficulties solved for this mining session every 5 passes
				// This will indicate the most common difficulty solved by this miner
				if (pass-1) % 5 == 0 {
					_current_ore_price=self.load_ore_price();
					_current_sol_price=self.load_sol_price();
					println!("\n{}", ("========================================================================================================================").to_string().dimmed());
					println!("| Current ORE Price: ${:.2}\tCurrent SOL Price: ${:.2}",
						_current_ore_price,
						_current_sol_price,
					);
					println!("| {}", max_reward_text);
					println!("| Average reward:     {:.11} ORE (${:.4}) over {} passes.",
						(session_ore_mined / (pass-1) as f64),
						(session_ore_mined / (pass-1) as f64) * _current_ore_price,
						pass-1,
					);
					println!("| Session Summary:\tProfit: ${:.4} ORE\t      Cost: ${:.4} SOL\tProfitablility: ${:.4}",
						session_ore_mined * _current_ore_price,
						session_sol_used * _current_sol_price,
						(session_ore_mined * _current_ore_price) - (session_sol_used * _current_sol_price),
					);

					println!("| Difficulties solved during {} passes:", pass-1);

					let mut max_count: u32 = 0;
					let mut most_popular_difficulty: u32 = 0;
					for (difficulty, count) in &difficulties_solved {
						if (*count as u32) >= max_count {
							max_count=*count as u32;
							most_popular_difficulty=*difficulty;
						}
						print!("|----");
					}
					println!("|");
					for (difficulty, _count) in &difficulties_solved {
						if *difficulty == most_popular_difficulty {
							print!("|{:>4}", difficulty.to_string().bold().yellow());
						} else {
							print!("|{:>4}", difficulty);
						}
					}
					println!("|");
					for (_difficulty, count) in &difficulties_solved {
						if (*count as u32) == max_count {
							print!("|{:>4}", (*count as u32).to_string().bold().yellow());
						} else {
							print!("|{:>4}", count);
						}
					}
					println!("|");
				} else {
					// Add a blank line if no summary is shown
					println!("")
				}
				println!("{}\n", ("========================================================================================================================").to_string().dimmed());
			}

			// Store this pass's sol/staked balances for use in the next pass
			last_sol_balance=current_sol_balance;
			last_staked_balance=current_staked_balance;

			// Lookup CPU stats for 1min, 5 mins and 15 mins
			let mut load_avg_1min: f32=0.0;
			let mut load_avg_5min: f32=0.0;
			let mut load_avg_15min: f32=0.0;
			match sys.load_average() {
				Ok(load_avg) => {
					load_avg_1min=load_avg.one;			// 1 min
					load_avg_5min=load_avg.five;		// 5 min
					load_avg_15min=load_avg.fifteen;	// 15 min
				}
				Err(err) => eprintln!("Error (load_average): {}", err),
			}
			// This will not report anything if in WSL2 on windows
			let cpu_temp: f32;
			match sys.cpu_temp() {
				Ok(t) => { cpu_temp=t; },
				Err(_err) => { cpu_temp=-99.0; },
					// eprintln!("Error (cpu_temp): {}", err),
			};
			let mut cpu_temp_txt=format!("{}°C   ", cpu_temp.to_string());
			if cpu_temp==-99.0 {
				cpu_temp_txt="".to_string();
			}
			// Write log details to console to summarize this miner's wallet
			println!("Pass {} started at {}\tMined for {}\tCPU: {}{:.2}/{:.2}/{:.2}",
				pass,
				Utc::now().format("%H:%M:%S on %Y-%m-%d").to_string(),
				format_duration(Duration::from_secs(mining_start_time.elapsed().as_secs())),
				cpu_temp_txt,
				load_avg_1min,
				load_avg_5min,
				load_avg_15min,
			);
			println!("        Currently staked ORE: {:.11}\tWallet SOL:  {:.9}\tLast Withdrawal: {:.1} hours ago {}",
				current_staked_balance,
				current_sol_balance,
				proof.last_claim_at,
				claim_text,
            );

			// Pause mining for one minute if no SOL available for transaction fee
			// This keeps the miner looping and will restart mining when enough SOL is added to miner's wallet
			if current_sol_balance<MIN_SOL_BALANCE {
				let progress_bar = Arc::new(spinner::new_progress_bar());
				for _ in 0..60 {
					progress_bar.set_message(format!("[{}{}] {}",
						(60-pass_start_time.elapsed().as_secs()).to_string().dimmed(),
						("s to go").dimmed(),
						("Not enough sol in wallet. Please deposit more to continue mining after the timeout.").yellow(),
					));
					std::thread::sleep(Duration::from_millis(1000));
				}
				progress_bar.finish_with_message(format!("[{}{}] {}",
					(60-pass_start_time.elapsed().as_secs()).to_string().dimmed(),
					("s").dimmed(),
					("Not enough sol in wallet. Please deposit more to continue mining.").yellow(),
				));
			}

			// The proof of work processing for this individual mining pass
			if current_sol_balance>=MIN_SOL_BALANCE {
				// Run drillx
				let (solution, best_difficulty) = self.find_hash_par(proof, cutoff_time, args.threads).await;

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
				self.send_and_confirm(&ixs, ComputeBudget::Fixed(500_000), false, true)
					.await
					.ok();

				// Log the difficulty solved to hashMap to record progress
				*difficulties_solved.entry(best_difficulty).or_insert(0) += 1;
				last_pass_difficulty=best_difficulty;
			}

			// Log how long this pass took to complete
			print!("  [{}{}] Completed",
				pass_start_time.elapsed().as_secs().to_string().dimmed(),
				"s".dimmed(),
			);
			pass+=1;
        }
    }

    async fn find_hash_par(&self, proof: Proof, cutoff_time: u64, threads: u64) -> (Solution, u32) {
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
											"[{}{}] Mining... {} {}",
											cutoff_time.saturating_sub(next_elapsed).to_string().dimmed(),
											"s to go".dimmed(),
											"Difficulty so far:".dimmed(),
											best_difficulty.to_string().yellow(),
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
            "[{}{}] Difficulty: {}\t    Hash: {} ",
			timer.elapsed().as_secs().to_string().dimmed(),
			"s".dimmed(),
            best_difficulty.to_string().bold().yellow(),
            bs58::encode(best_hash.h).into_string().dimmed(),
        ));

        (Solution::new(best_hash.d, best_nonce.to_le_bytes()), best_difficulty)
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

	// Determine if a reset is required ()
    async fn needs_reset(&self) -> bool {
        let clock = get_clock(&self.rpc_client).await;
        let config = get_config(&self.rpc_client).await;
        config
            .last_reset_at
            .saturating_add(EPOCH_DURATION)
            .saturating_sub(5) // Buffer
            .le(&clock.unix_timestamp)
    }

	// Calculate how long to mine for
	// Based upon (last_hash_at time) + (1 minute) - (desired buffer_time) - (clock time)
    async fn get_cutoff(&self, proof: Proof, buffer_time: u64, clock: &Clock) -> u64 {
        // clock is passed in to prevent calling get_clock() twice in quick succession from RPC
		// let clock = get_clock(&self.rpc_client).await;
        let mut retval=proof.last_hash_at
            .saturating_add(60)
            .saturating_sub(buffer_time as i64)
            .saturating_sub(clock.unix_timestamp.clone())
            .max(0) as u64;
		if retval==0 {
			retval=(60 as i64).saturating_sub(buffer_time as i64).max(0) as u64;
		}
		return retval;
    }

	// Request the price of SOL from coinmarketcap
	// async fn download_sol_price(&self) {
	// 	let url = "https://api.coinmarketcap.com/data/price?ids=solana&convert=USD&x_cg_demo_api_key=CG-DWoujcLEMj2Pk6omqJaJsKLZ";
	// 	// SOL_PRICE=$(curl -s "https://api.coingecko.com/api/v3/simple/price?ids=solana&vs_currencies=usd&x_cg_demo_api_key=${COINGECKO_APIKEY}" | jq '.solana.usd')
	// 	// let resp = reqwest::blocking::get(url)?.text()?;
	// 	// println!("Coinmarketcap SOL price: {}", resp);
	// }

	// Query the wallet for the amount of SOL present and panic if less than a minimum amount
	async fn get_sol_balance(&self, panic: bool) -> f64 {
		const MIN_SOL_BALANCE: f64 = 0.005;
		let signer = self.signer();
		let client = self.rpc_client.clone();

		// Return error, if balance is zero
		if let Ok(lamports_balance) = client.get_balance(&signer.pubkey()).await {
			let sol_balance:f64 = lamports_to_sol(lamports_balance);
			if lamports_balance <= sol_to_lamports(MIN_SOL_BALANCE) {
				if panic {
					panic!(
						"{} Insufficient balance: {} SOL\nPlease top up with at least {} SOL",
						"ERROR".bold().red(),
						sol_balance,
						MIN_SOL_BALANCE
					);
				}
			}
			return sol_balance
		} else {
			if panic {
				panic!(
					"{} Failed to lookup sol balance",
					"ERROR".bold().red(),
				);
			} else {
				return 0.0 as f64
			}
		}
	}

	// Read a file to get a f64 value from the first line of the file
	pub fn read_f64_from_file(&self, file_path: &str) -> Result<f64> {
		let file = File::open(file_path)?;
		let reader = BufReader::new(file);

		if let Some(first_line) = reader.lines().next() {
			if let Ok(line) = first_line {
				if let Ok(value) = line.trim().parse::<f64>() {
					return Ok(value);
				}
			}
		}

		Err(std::io::Error::new(
			std::io::ErrorKind::InvalidData,
			"Failed to read or parse f64 from the first line.",
		))
	}

	// read the current ORE price in from text file
	fn load_ore_price(&self) -> f64 {
		let file_path = "./currentPriceOfOre.txt";
		match self.read_f64_from_file(&file_path) {
			Ok(value) => value,
			Err(err) => {
				eprintln!("Error: failed to read ORE price from {}: {}", file_path, err);
				0.0
			}
		}
	}

	// read the current SOL price in from text file
	fn load_sol_price(&self) -> f64 {
		let file_path = "./currentPriceOfSol.txt";
		match self.read_f64_from_file(&file_path) {
			Ok(value) => value,
			Err(err) => {
				eprintln!("Error: failed to read SOL price from {}: {}", file_path, err);
				0.0
			}
		}
	}

}

// TODO Pick a better strategy (avoid draining bus)
fn find_bus() -> Pubkey {
    let i = rand::thread_rng().gen_range(0..BUS_COUNT);
    BUS_ADDRESSES[i]
}
