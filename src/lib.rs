pub mod trex;

use serde::{Deserialize, Serialize, Serializer};

pub type ClientId = u16;
pub type Coins = rust_decimal::Decimal;
pub type TransactionId = u32;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum TransactionType {
    Deposit,
    Withdrawal,
    Dispute,
    Resolve,
    Chargeback,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transaction {
    pub r#type: TransactionType,
    pub client: ClientId,
    pub tx: TransactionId,
    pub amount: Coins,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Client {
    pub client: ClientId,
    #[serde(serialize_with = "rounded")]
    pub available: Coins,
    #[serde(serialize_with = "rounded")]
    pub held: Coins,
    #[serde(serialize_with = "rounded")]
    pub total: Coins,
    pub locked: bool,
}

fn rounded<S: Serializer>(value: &Coins, serializer: S) -> Result<S::Ok, S::Error> {
    serializer.serialize_str(&value.round_dp(4).to_string())
}
