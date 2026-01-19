use serde::Serialize;

use tokio::sync::broadcast;

const CHANNEL_CAPACITY: usize = 256;

#[derive(Debug, Clone, Serialize)]
pub struct BlockUpdate {
    pub chain_id: u64,
    pub block_num: u64,
    pub block_hash: String,
    pub tx_count: u64,
    pub log_count: u64,
    pub timestamp: i64,
}

#[derive(Clone)]
pub struct Broadcaster {
    sender: broadcast::Sender<BlockUpdate>,
}

impl Default for Broadcaster {
    fn default() -> Self {
        Self::new()
    }
}

impl Broadcaster {
    pub fn new() -> Self {
        let (sender, _) = broadcast::channel(CHANNEL_CAPACITY);
        Self { sender }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<BlockUpdate> {
        self.sender.subscribe()
    }

    pub fn send(&self, update: BlockUpdate) {
        let _ = self.sender.send(update);
    }

    pub fn receiver_count(&self) -> usize {
        self.sender.receiver_count()
    }
}
