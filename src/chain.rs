//! Chain wire type boundary used by sync/fetch/decode.
//!
//! Callers import from `crate::chain` so sync/fetch/decode stays chain-agnostic.

pub type Block = alloy::rpc::types::Block<Transaction>;
pub type Transaction = alloy::rpc::types::Transaction;
pub type Log = alloy::rpc::types::Log;
pub type Receipt = alloy::rpc::types::TransactionReceipt;
