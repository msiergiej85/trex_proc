pub mod trex;

use serde::{Deserialize, Serialize};

pub type ClientId = u16;
pub type Coins = f32;

#[derive(Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum TransactionType {
    Deposit,
    Withdrawal,
    Dispute,
    Resolve,
    Chargeback,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Transaction {
    pub r#type: TransactionType,
    pub client: ClientId,
    pub tx: u32,
    pub amount: Coins,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Client {
    pub client: ClientId,
    pub available: Coins,
    pub held: Coins,
    pub total: Coins,
    pub locked: bool,
}
