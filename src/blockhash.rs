use std::{sync::Arc, time::Duration};

use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::hash::Hash;
use tokio::sync::RwLock;

pub struct BlockhashService;

const POLL_MILLIS: u64 = 10_000;

impl BlockhashService {
    pub fn start(rpc: Arc<RpcClient>) -> LatestBlockhash {
        // Polling interval
        let mut interval = tokio::time::interval(Duration::from_millis(POLL_MILLIS));

        // Arc value
        let blockhash_arc = Arc::new(RwLock::new(None));
        let blockhash_arc_ = blockhash_arc.clone();

        // Spawn polling task
        tokio::task::spawn(async move {
            let mut attempts = 0;
            loop {
                interval.tick().await;

                // Fetch data
                let Ok(hash) = rpc.get_latest_blockhash().await else {
                    attempts += 1;
                    logfather::warn!("failed to fetch blockhash ({})", attempts);
                    continue;
                };

                // Update arc
                *blockhash_arc_.write().await = Some(hash);

                // Reset attempts
                attempts = 0;
            }
        });

        LatestBlockhash(blockhash_arc)
    }
}

#[derive(Clone)]
pub struct LatestBlockhash(Arc<RwLock<Option<Hash>>>);

impl LatestBlockhash {
    pub async fn load(&self) -> Option<Hash> {
        *self.0.read().await
    }
}
