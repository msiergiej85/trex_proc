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

/// Note: Methods are async for future-proofing (wrap HashMap in RWLock and Clients as well,
/// put mpsc channel between process_transaction() call task actually processing it).
impl Trex {
    pub async fn process_transaction(&mut self, transaction: Transaction) {
        match self.clients.entry(transaction.client) {
            Entry::Occupied(entry) => {
                entry.into_mut().handle_transaction(transaction.into());
            }
            Entry::Vacant(entry) => {
                let mut client = ClientState {
                    state: Client {
                        client: transaction.client,
                        available: 0.0,
                        held: 0.0,
                        total: 0.0,
                        locked: false,
                    },
                    history: Vec::new(),
                };
                if client.handle_transaction(transaction.into()) {
                    entry.insert(client);
                }
            }
        }
    }

    pub async fn clients(&self) -> Vec<ClientId> {
        self.clients.keys().cloned().collect()
    }

    pub async fn client(&self, id: ClientId) -> Option<Client> {
        self.clients.get(&id).map(|client| client.state.clone())
    }
}

impl ClientState {
    fn handle_transaction(&mut self, tx: Tx) -> bool {
        if self.state.locked {
            info!(
                "Transaction rejected - client {} is locked",
                self.state.client
            );
            return false;
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
                        "Dispute for {} rejected - associated transaction {} not found",
                        self.state.client, tx.id
                    );
                    return false;
                };
                self.state.available -= coins;
                self.state.held += coins;
            }
            TransactionType::Resolve => {
                let Some(coins) = self.find_tx(tx.id) else {
                    info!(
                        "Resolve for {} rejected - associated transaction {} not found",
                        self.state.client, tx.id
                    );
                    return false;
                };
                if !self.has_open_dispute(tx.id) {
                    info!(
                        "Resolve for {} rejected - no dispute open for associated transaction {}",
                        self.state.client, tx.id
                    );
                    return false;
                }
                self.state.available += coins;
                self.state.held -= coins;
            }
            TransactionType::Chargeback => {
                let Some(coins) = self.find_tx(tx.id) else {
                    info!(
                        "Chargeback for {} rejected - associated transaction {} not found",
                        self.state.client, tx.id
                    );
                    return false;
                };
                if !self.has_open_dispute(tx.id) {
                    info!(
                        "Chargeback for {} rejected - no dispute open for associated transaction {}",
                        self.state.client, tx.id
                    );
                    return false;
                }
                self.state.held -= coins;
                self.state.total -= coins;
                self.state.locked = true;
            }
        }
        self.store_tx(tx);
        true
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

    fn has_open_dispute(&self, id: u32) -> bool {
        self.history
            .iter()
            .rfind(|tx| tx.id == id)
            .map(|tx| tx.r#type == TransactionType::Dispute)
            .unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TransactionType;

    #[tokio::test]
    async fn test_basic_operations() {
        let mut trex = Trex::default();

        // single deposit
        trex.process_transaction(Transaction {
            r#type: TransactionType::Deposit,
            client: 1,
            tx: 1,
            amount: 100.0,
        })
        .await;

        let client = trex.client(1).await.unwrap();
        assert_eq!(client.client, 1);
        assert_eq!(client.available, 100.0);
        assert_eq!(client.total, 100.0);
        assert_eq!(client.held, 0.0);
        assert!(!client.locked);

        trex.process_transaction(Transaction {
            r#type: TransactionType::Deposit,
            client: 1,
            tx: 1,
            amount: 100.0,
        })
        .await;
        trex.process_transaction(Transaction {
            r#type: TransactionType::Withdrawal,
            client: 1,
            tx: 2,
            amount: 50.0,
        })
        .await;

        let client = trex.client(1).await.unwrap();
        assert_eq!(client.available, 150.0);
        assert_eq!(client.total, 150.0);

        trex.process_transaction(Transaction {
            r#type: TransactionType::Withdrawal,
            client: 2,
            tx: 2,
            amount: 200.0,
        })
        .await;
        trex.process_transaction(Transaction {
            r#type: TransactionType::Deposit,
            client: 2,
            tx: 3,
            amount: 150.0,
        })
        .await;

        let client = trex.client(2).await.unwrap();
        assert_eq!(client.total, -50.0);
    }

    #[tokio::test]
    async fn test_dispute() {
        let mut trex = Trex::default();
        trex.process_transaction(Transaction {
            r#type: TransactionType::Deposit,
            client: 1,
            tx: 1,
            amount: 100.0,
        })
        .await;
        trex.process_transaction(Transaction {
            r#type: TransactionType::Dispute,
            client: 1,
            tx: 1,
            amount: 0.0,
        })
        .await;

        let client = trex.client(1).await.unwrap();
        assert_eq!(client.available, 0.0);
        assert_eq!(client.held, 100.0);
        assert_eq!(client.total, 100.0);

        trex.process_transaction(Transaction {
            r#type: TransactionType::Deposit,
            client: 1,
            tx: 2,
            amount: 10.0,
        })
        .await;

        trex.process_transaction(Transaction {
            r#type: TransactionType::Resolve,
            client: 1,
            tx: 1,
            amount: 0.0,
        })
        .await;

        let client = trex.client(1).await.unwrap();
        assert_eq!(client.available, 110.0);
        assert_eq!(client.held, 0.0);
        assert_eq!(client.total, 110.0);

        trex.process_transaction(Transaction {
            r#type: TransactionType::Dispute,
            client: 1,
            tx: 2,
            amount: 0.0,
        })
        .await;
        trex.process_transaction(Transaction {
            r#type: TransactionType::Chargeback,
            client: 1,
            tx: 2,
            amount: 0.0,
        })
        .await;

        let client = trex.client(1).await.unwrap();
        assert_eq!(client.available, 100.0);
        assert_eq!(client.held, 0.0);
        assert_eq!(client.total, 100.0);
        assert!(client.locked);
    }

    #[tokio::test]
    async fn test_ignore_invalid_transactions() {
        let mut trex = Trex::default();
        trex.process_transaction(Transaction {
            r#type: TransactionType::Deposit,
            client: 1,
            tx: 1,
            amount: 100.0,
        })
        .await;
        trex.process_transaction(Transaction {
            r#type: TransactionType::Deposit,
            client: 2,
            tx: 1,
            amount: 10.0,
        })
        .await;
        trex.process_transaction(Transaction {
            r#type: TransactionType::Dispute,
            client: 2,
            tx: 1,
            amount: 0.0,
        })
        .await;
        trex.process_transaction(Transaction {
            r#type: TransactionType::Chargeback,
            client: 2,
            tx: 1,
            amount: 0.0,
        })
        .await;

        let client = trex.client(2).await.unwrap();
        assert_eq!(client.total, 0.0);
        assert!(client.locked);

        // ignore deposit on locked client
        trex.process_transaction(Transaction {
            r#type: TransactionType::Deposit,
            client: 2,
            tx: 2,
            amount: 50.0,
        })
        .await;
        assert_eq!(trex.client(2).await.unwrap().total, 0.0);

        // ignore dispute on non-existing client
        trex.process_transaction(Transaction {
            r#type: TransactionType::Dispute,
            client: 3,
            tx: 2,
            amount: 50.0,
        })
        .await;
        assert!(trex.client(3).await.is_none());

        // ignore dispute on non-existing transaction
        trex.process_transaction(Transaction {
            r#type: TransactionType::Dispute,
            client: 3,
            tx: 2,
            amount: 50.0,
        })
        .await;
        assert!(trex.client(3).await.is_none());

        // ignore chargeback on transaction without open dispute
        trex.process_transaction(Transaction {
            r#type: TransactionType::Chargeback,
            client: 1,
            tx: 1,
            amount: 0.0,
        })
        .await;
        assert_eq!(trex.client(1).await.unwrap().available, 100.0);

        // ignore resolve on transaction without open dispute
        trex.process_transaction(Transaction {
            r#type: TransactionType::Resolve,
            client: 1,
            tx: 1,
            amount: 0.0,
        })
        .await;
        assert_eq!(trex.client(1).await.unwrap().available, 100.0);
    }
}
