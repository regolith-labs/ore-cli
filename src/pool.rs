use std::sync::Arc;

use drillx::Solution;
use ore_pool_types::{ContributePayload, Member, MemberChallenge, PoolAddress, RegisterPayload};
use ore_utils::AccountDeserialize;
use solana_rpc_client::spinner;
use solana_sdk::{pubkey::Pubkey, signature::Signature, signer::Signer};

use crate::{error::Error, Miner};

pub struct Pool {
    pub http_client: reqwest::Client,
    pub pool_url: String,
}

impl Pool {
    // TODO: build and sign tx here
    pub async fn post_pool_register(&self, miner: &Miner) -> Result<Member, Error> {
        let pubkey = miner.signer().pubkey();
        let post_url = format!("{}/register", self.pool_url);
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
