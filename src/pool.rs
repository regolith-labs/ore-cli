use std::sync::Arc;

use drillx::Solution;
use ore_pool_types::{
    BalanceUpdate, ContributePayload, Member, MemberChallenge, PoolAddress, RegisterPayload,
    RegisterStakerPayload, Staker, UpdateBalancePayload,
};
use solana_rpc_client::spinner;
use solana_sdk::{
    compute_budget, pubkey::Pubkey, signature::Signature, signer::Signer, transaction::Transaction,
};
use steel::AccountDeserialize;

use crate::{cu_limits::CU_LIMIT_CLAIM, error::Error, send_and_confirm::ComputeBudget, Miner};

pub struct Pool {
    pub http_client: reqwest::Client,
    pub pool_url: String,
}

impl Pool {
    pub async fn post_pool_register(&self, miner: &Miner) -> Result<Member, Error> {
        let pubkey = miner.signer().pubkey();
        let post_url = format!("{}/register", self.pool_url);
        // check if on-chain member account exists already
        let pool_pda = self.get_pool_address().await?;
        if let Err(_err) = self.get_pool_member_onchain(miner, pool_pda.address).await {
            // on-chain member account not found
            // create one before submitting register payload to pool
            let ix = ore_pool_api::sdk::join(pubkey, pool_pda.address, pubkey);
            let _ = miner
                .send_and_confirm(&[ix], ComputeBudget::Fixed(200_000), false)
                .await?;
        };
        // submit idempotent register payload
        // will simply return off-chain account if already registered
        let body = RegisterPayload { authority: pubkey };
        self.http_client
            .post(post_url)
            .json(&body)
            .send()
            .await?
            .json::<Member>()
            .await
            .map_err(From::from)
    }

    pub async fn post_pool_register_staker(
        &self,
        miner: &Miner,
        mint: &Pubkey,
    ) -> Result<Staker, Error> {
        let pubkey = miner.signer().pubkey();
        let post_url = format!("{}/register-staker", self.pool_url);
        // check if on-chain share account exists already
        let pool_address = self.get_pool_address().await?;
        if let Err(_err) = self
            .get_staker_onchain(miner, pool_address.address, *mint)
            .await
        {
            println!("creating new share account");
            // on-chain staker account not found
            // create one before submitting register payload to pool
            let ix = ore_pool_api::sdk::open_share(pubkey, *mint, pool_address.address);
            let _ = miner
                .send_and_confirm(&[ix], ComputeBudget::Fixed(CU_LIMIT_CLAIM), false)
                .await?;
            // sleep to allow the rpc connection on the pool server to catch up
            tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
        }
        // submit idempotent regiter payload
        // will simply return off-chain account if already registered
        let body = RegisterStakerPayload {
            authority: pubkey,
            mint: *mint,
        };
        self.http_client
            .post(post_url)
            .json(&body)
            .send()
            .await?
            .json::<Staker>()
            .await
            .map_err(From::from)
    }

    pub async fn get_pool_address(&self) -> Result<PoolAddress, Error> {
        let get_url = format!("{}/pool-address", self.pool_url);
        self.http_client
            .get(get_url)
            .send()
            .await?
            .json::<PoolAddress>()
            .await
            .map_err(From::from)
    }

    pub async fn get_pool_member_onchain(
        &self,
        miner: &Miner,
        pool_address: Pubkey,
    ) -> Result<ore_pool_api::state::Member, Error> {
        let (member_pda, _) =
            ore_pool_api::state::member_pda(miner.signer().pubkey(), pool_address);
        let data = miner.rpc_client.get_account_data(&member_pda).await?;
        let pool = ore_pool_api::state::Member::try_from_bytes(data.as_slice())?;
        Ok(*pool)
    }

    pub async fn get_staker_onchain(
        &self,
        miner: &Miner,
        pool_address: Pubkey,
        mint: Pubkey,
    ) -> Result<ore_pool_api::state::Share, Error> {
        let (share_pda, _) =
            ore_pool_api::state::share_pda(miner.signer().pubkey(), pool_address, mint);
        let data = miner.rpc_client.get_account_data(&share_pda).await?;
        let share = ore_pool_api::state::Share::try_from_bytes(data.as_slice())?;
        Ok(*share)
    }

    pub async fn get_pool_member(&self, miner: &Miner) -> Result<Member, Error> {
        let pubkey = miner.signer().pubkey();
        let get_url = format!("{}/member/{}", self.pool_url, pubkey);
        self.http_client
            .get(get_url)
            .send()
            .await?
            .json::<Member>()
            .await
            .map_err(From::from)
    }

    pub async fn get_updated_pool_challenge(
        &self,
        last_hash_at: i64,
    ) -> Result<MemberChallenge, Error> {
        let mut retries = 0;
        let max_retries = 24; // 120 seconds, should yield new challenge
        let progress_bar = Arc::new(spinner::new_progress_bar());
        loop {
            progress_bar.set_message("Fetching new challenge...");
            let challenge = self.get_pool_challenge().await?;
            if challenge.challenge.lash_hash_at == last_hash_at {
                retries += 1;
                if retries == max_retries {
                    return Err(Error::Internal("could not fetch new challenge".to_string()));
                }
                tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
            } else {
                progress_bar.finish_with_message("Found new challenge");
                return Ok(challenge);
            }
        }
    }

    pub async fn post_update_balance(&self, miner: &Miner) -> Result<(), Error> {
        let signer = &miner.signer();
        let signer_pubkey = &signer.pubkey();
        let post_url = format!("{}/update-balance", self.pool_url);
        // fetch member balance
        let member = self.get_pool_member(miner).await?;
        // fetch pool for authority
        let pool = self.get_pool_address().await?;
        let data = miner.rpc_client.get_account_data(&pool.address).await?;
        let pool = ore_pool_api::state::Pool::try_from_bytes(data.as_slice())?;
        let pool_authority = pool.authority;
        // build attribute instruction
        let ix = ore_pool_api::sdk::attribute(
            pool_authority,
            *signer_pubkey,
            member.total_balance as u64,
        );
        let compute_budget_limit_ix =
            compute_budget::ComputeBudgetInstruction::set_compute_unit_limit(100_000);
        let compute_budget_price_ix =
            compute_budget::ComputeBudgetInstruction::set_compute_unit_price(20_000);
        let mut tx = Transaction::new_with_payer(
            &[compute_budget_limit_ix, compute_budget_price_ix, ix],
            Some(signer_pubkey),
        );
        let hash = miner.rpc_client.get_latest_blockhash().await?;
        tx.partial_sign(&[signer], hash);
        // build payload
        let paylaod = UpdateBalancePayload {
            authority: *signer_pubkey,
            transaction: tx,
            hash,
        };
        // post
        let balance_update = self
            .http_client
            .post(post_url)
            .json(&paylaod)
            .send()
            .await?
            .json::<BalanceUpdate>()
            .await;
        println!("{:?}", balance_update);
        Ok(())
    }

    async fn get_pool_challenge(&self) -> Result<MemberChallenge, Error> {
        let get_url = format!("{}/challenge", self.pool_url);
        let resp = self.http_client.get(get_url).send().await?;
        resp.json::<MemberChallenge>().await.map_err(From::from)
    }

    pub async fn post_pool_solution(
        &self,
        miner: &Miner,
        solution: &Solution,
    ) -> Result<(), Error> {
        let pubkey = miner.signer().pubkey();
        let signature = Pool::sign_solution(miner, solution);
        let payload = ContributePayload {
            authority: pubkey,
            solution: *solution,
            signature,
        };
        let post_url = format!("{}/contribute", self.pool_url);
        let _ = self.http_client.post(post_url).json(&payload).send().await;
        Ok(())
    }

    fn sign_solution(miner: &Miner, solution: &Solution) -> Signature {
        let keypair = &miner.signer();
        keypair.sign_message(solution.to_bytes().as_slice())
    }
}
