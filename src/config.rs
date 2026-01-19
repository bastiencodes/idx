pub struct Config {
    pub rpc_url: String,
    pub database_url: String,
}

pub const PRESTO_CHAIN_ID: u64 = 4217;
pub const ANDANTINO_CHAIN_ID: u64 = 42429;
pub const MODERATO_CHAIN_ID: u64 = 42431;

#[derive(Debug, Clone)]
pub struct ChainConfig {
    pub name: &'static str,
    pub chain_id: u64,
    pub rpc_url: &'static str,
}

pub const CHAINS: &[ChainConfig] = &[
    ChainConfig {
        name: "presto",
        chain_id: PRESTO_CHAIN_ID,
        rpc_url: "https://rpc.presto.tempo.xyz",
    },
    ChainConfig {
        name: "andantino",
        chain_id: ANDANTINO_CHAIN_ID,
        rpc_url: "https://rpc.testnet.tempo.xyz",
    },
    ChainConfig {
        name: "moderato",
        chain_id: MODERATO_CHAIN_ID,
        rpc_url: "https://rpc.moderato.tempo.xyz",
    },
];

pub fn get_chain(name: &str) -> Option<&'static ChainConfig> {
    let name_lower = name.to_lowercase();
    CHAINS.iter().find(|c| c.name == name_lower)
}

pub fn chain_name(chain_id: u64) -> &'static str {
    CHAINS
        .iter()
        .find(|c| c.chain_id == chain_id)
        .map(|c| c.name)
        .unwrap_or("unknown")
}

pub fn list_chains() -> &'static [ChainConfig] {
    CHAINS
}
