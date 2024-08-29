use drillx::Solution;
use ore_pool_types::{ContributePayload, MemberChallenge};
use solana_sdk::{signature::Signature, signer::Signer};

use crate::{error::Error, Miner};

impl Miner {
    pub async fn get_updated_pool_challenge(
        &self,
        http_client: &reqwest::Client,
        last_hash_at: i64,
    ) -> Result<MemberChallenge, Error> {
        let mut retries = 0;
        let max_retries = 12; // 60 seconds, should yield new challenge
        loop {
            let challenge = self.get_pool_challenge(http_client).await?;
            println!("fetched: {:?}", challenge.challenge.lash_hash_at);
            println!("live: {:?}", last_hash_at);
            if challenge.challenge.lash_hash_at == last_hash_at {
                retries += 1;
                if retries == max_retries {
                    return Err(Error::Internal("could not fetch new challenge".to_string()));
                }
                tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
            } else {
                return Ok(challenge);
            }
        }
    }

    async fn get_pool_challenge(
        &self,
        http_client: &reqwest::Client,
    ) -> Result<MemberChallenge, Error> {
        let pool_url = &self.pool_url.clone().ok_or(Error::Internal(
            "must specify the pool url flag".to_string(),
        ))?;
        let pubkey = self.signer().pubkey();
        let get_url = format!("{}/challenge/{}", pool_url, pubkey);
        let resp = http_client.get(get_url).send().await?;
        resp.json::<MemberChallenge>().await.map_err(From::from)
    }

    pub async fn post_pool_solution(
        &self,
        http_client: &reqwest::Client,
        solution: &Solution,
    ) -> Result<(), Error> {
        let pool_url = &self.pool_url.clone().ok_or(Error::Internal(
            "must specify the pool url flag".to_string(),
        ))?;
        let pubkey = self.signer().pubkey();
        let signature = self.sign_solution(solution);
        let payload = ContributePayload {
            authority: pubkey,
            solution: *solution,
            signature,
        };
        let post_url = format!("{}/contribute", pool_url);
        match http_client.post(post_url).json(&payload).send().await {
            Ok(resp) => {
                println!("resp: {:?}", resp);
            }
            Err(err) => {
                println!("{:?}", err);
            }
        };
        Ok(())
    }

    fn sign_solution(&self, solution: &Solution) -> Signature {
        let keypair = &self.signer();
        keypair.sign_message(solution.to_bytes().as_slice())
    }
}
