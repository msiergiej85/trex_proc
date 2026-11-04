use crate::{Client, ClientId, Coins, Transaction, TransactionType};
use std::collections::HashMap;

struct Tx {
    pub id: u32,
    pub r#type: TransactionType,
    pub amount: Coins,
}

impl From<Transaction> for Tx {
    fn from(value: Transaction) -> Self {
        Self {
            id: value.tx,
            r#type: value.r#type,
            amount: value.amount,
        }
    }
}

struct ClientState {
    state: Client,
    history: Vec<Tx>,
}

#[derive(Default)]
pub struct Trex {
    clients: HashMap<ClientId, ClientState>,
}

/// Note: Methods are async for future-proofing (wrap HashMap in RWLock and Clients as well).
impl Trex {
    pub async fn process_transaction(&mut self, _transaction: Transaction) {
        // ToDo: implement
    }

    pub async fn clients(&self) -> Vec<ClientId> {
        self.clients.keys().cloned().collect()
    }

    pub async fn client(&self, id: ClientId) -> Option<Client> {
        self.clients.get(&id).map(|client| client.state.clone())
    }
}
