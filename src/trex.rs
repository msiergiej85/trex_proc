use crate::{Client, ClientId, Coins, Transaction, TransactionType};
use log::info;
use std::collections::{HashMap, hash_map::Entry};

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
    pub async fn process_transaction(&mut self, transaction: Transaction) {
        let client = match self.clients.entry(transaction.client) {
            Entry::Occupied(entry) => entry.into_mut(),
            Entry::Vacant(entry) => entry.insert(ClientState {
                state: Client {
                    client: transaction.client,
                    available: 0.0,
                    held: 0.0,
                    total: 0.0,
                    locked: false,
                },
                history: Vec::new(),
            }),
        };
        client.handle_transaction(transaction.into());
    }

    pub async fn clients(&self) -> Vec<ClientId> {
        self.clients.keys().cloned().collect()
    }

    pub async fn client(&self, id: ClientId) -> Option<Client> {
        self.clients.get(&id).map(|client| client.state.clone())
    }
}

impl ClientState {
    fn handle_transaction(&mut self, tx: Tx) {
        if self.state.locked {
            info!(
                "Transaction rejected  - client {} is locked",
                self.state.client
            );
            return;
        }

        match tx.r#type {
            TransactionType::Deposit => {
                self.state.available += tx.amount;
                self.state.total += tx.amount;
            }
            TransactionType::Withdrawal => {
                self.state.available -= tx.amount;
                self.state.total -= tx.amount;
            }
            TransactionType::Dispute => {
                let Some(coins) = self.find_tx(tx.id) else {
                    info!(
                        "Dispute for {} rejected  - associated transaction {} not found",
                        self.state.client, tx.id
                    );
                    return;
                };
                self.state.available -= coins;
                self.state.held += coins;
            }
            TransactionType::Resolve => {
                let Some(coins) = self.find_tx(tx.id) else {
                    info!(
                        "Resolve for {} rejected  - associated transaction {} not found",
                        self.state.client, tx.id
                    );
                    return;
                };
                self.state.available += coins;
                self.state.held -= coins;
            }
            TransactionType::Chargeback => {
                let Some(coins) = self.find_tx(tx.id) else {
                    info!(
                        "Chargeback for {} rejected  - associated transaction {} not found",
                        self.state.client, tx.id
                    );
                    return;
                };
                self.state.held -= coins;
                self.state.total -= coins;
                self.state.locked = true;
            }
        }
        self.store_tx(tx);
    }

    fn store_tx(&mut self, tx: Tx) {
        self.history.push(tx);
    }

    fn find_tx(&self, id: u32) -> Option<Coins> {
        self.history
            .iter()
            .rfind(|tx| {
                tx.id == id
                    && matches!(
                        tx.r#type,
                        TransactionType::Deposit | TransactionType::Withdrawal
                    )
            })
            .map(|tx| tx.amount)
    }
}
