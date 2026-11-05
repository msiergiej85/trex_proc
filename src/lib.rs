pub mod trex;

use derive_more::{Add, AddAssign, Deref, DerefMut, Sub, SubAssign};
use serde::{Deserialize, Serialize, Serializer};
use std::cmp::Ordering;

/// Transaction value type wrapper. Just in case if precision requirements would change.
#[derive(
    Debug,
    Serialize,
    Deserialize,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Deref,
    DerefMut,
    Add,
    AddAssign,
    Sub,
    SubAssign,
)]
pub struct Coins(fixdec::D64);

impl Coins {
    pub fn new(mantissa: i64, scale: u32) -> Self {
        Self(fixdec::D64::with_scale(mantissa, scale))
    }
}

impl TryFrom<f32> for Coins {
    type Error = ();

    fn try_from(value: f32) -> Result<Self, Self::Error> {
        fixdec::D64::from_f32(value).map(Coins).ok_or_else(|| ())
    }
}

impl PartialOrd for Coins {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.0.partial_cmp(&other.0)
    }
}

pub type ClientId = u16;
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

/// Rust representation of transaction, according to spec.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transaction {
    pub r#type: TransactionType,
    pub client: ClientId,
    pub tx: TransactionId,
    pub amount: Coins,
}

/// Rust representation of client account state, according to spec.
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
