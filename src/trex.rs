use crate::{Client, ClientId, Coins, Transaction, TransactionId, TransactionType};
use log::info;
use std::collections::{HashMap, hash_map::Entry};

struct Tx {
    pub id: TransactionId,
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

/// Internal representation of Client account.
struct ClientState {
    /// Current account state.
    state: Client,
    /// All transactions' history.
    history: Vec<Tx>,
}

/// `Trex` Transactions processor. Handle transactions and maintain account state for group of Clients.
/// It is up to user how clients are distributed over `Trex` instances, but once first transaction
/// for some client is processed by `Trex` then user must guarantee that all other transactions associated
/// with given client will go to the same instance of `Trex`.
#[derive(Default)]
pub struct Trex {
    clients: HashMap<ClientId, ClientState>,
}

impl Trex {
    /// Validate transaction and update Client account state accordingly.
    pub fn process_transaction(&mut self, transaction: Transaction) {
        match self.clients.entry(transaction.client) {
            Entry::Occupied(entry) => {
                entry.into_mut().handle_transaction(transaction.into());
            }
            Entry::Vacant(entry) => {
                let mut client = ClientState {
                    state: Client {
                        client: transaction.client,
                        available: Coins::new(0, 0),
                        held: Coins::new(0, 0),
                        total: Coins::new(0, 0),
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

    /// Get list of Client ids maintained by this `Trex` so far.
    pub fn clients(&self) -> Vec<ClientId> {
        self.clients.keys().cloned().collect()
    }

    /// Get current Client account state, or `None` if not found.
    pub fn client(&self, id: ClientId) -> Option<Client> {
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
                if self.state.available < tx.amount {
                    info!(
                        "Withdrawal ({}) for {} rejected - not enough funds",
                        tx.id, self.state.client,
                    );
                    return false;
                }
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

    fn find_tx(&self, id: TransactionId) -> Option<Coins> {
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

    fn has_open_dispute(&self, id: TransactionId) -> bool {
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

        trex.process_transaction(Transaction {
            r#type: TransactionType::Deposit,
            client: 1,
            tx: 1,
            amount: Coins::new(10001, 2),
        });

        let client = trex.client(1).unwrap();
        assert_eq!(client.client, 1);
        assert_eq!(client.available.to_f32(), 100.01);
        assert_eq!(client.total.to_f32(), 100.01);
        assert_eq!(client.held.to_f32(), 0.0);
        assert!(!client.locked);

        trex.process_transaction(Transaction {
            r#type: TransactionType::Deposit,
            client: 1,
            tx: 2,
            amount: Coins::new(100, 0),
        });
        trex.process_transaction(Transaction {
            r#type: TransactionType::Withdrawal,
            client: 1,
            tx: 3,
            amount: Coins::new(50, 0),
        });

        let client = trex.client(1).unwrap();
        assert_eq!(client.available.to_f32(), 150.01);
        assert_eq!(client.total.to_f32(), 150.01);

        trex.process_transaction(Transaction {
            r#type: TransactionType::Deposit,
            client: 2,
            tx: 4,
            amount: Coins::new(200, 0),
        });
        trex.process_transaction(Transaction {
            r#type: TransactionType::Withdrawal,
            client: 2,
            tx: 5,
            amount: Coins::new(50, 0),
        });

        let client = trex.client(2).unwrap();
        assert_eq!(client.total.to_f32(), 150.0);
    }

    #[tokio::test]
    async fn test_dispute() {
        let mut trex = Trex::default();
        trex.process_transaction(Transaction {
            r#type: TransactionType::Deposit,
            client: 1,
            tx: 1,
            amount: Coins::new(100, 0),
        });
        trex.process_transaction(Transaction {
            r#type: TransactionType::Dispute,
            client: 1,
            tx: 1,
            amount: Coins::new(0, 0),
        });

        let client = trex.client(1).unwrap();
        assert_eq!(client.available.to_f32(), 0.0);
        assert_eq!(client.held.to_f32(), 100.0);
        assert_eq!(client.total.to_f32(), 100.0);

        trex.process_transaction(Transaction {
            r#type: TransactionType::Deposit,
            client: 1,
            tx: 2,
            amount: Coins::new(10, 0),
        });

        trex.process_transaction(Transaction {
            r#type: TransactionType::Resolve,
            client: 1,
            tx: 1,
            amount: Coins::new(0, 0),
        });

        let client = trex.client(1).unwrap();
        assert_eq!(client.available.to_f32(), 110.0);
        assert_eq!(client.held.to_f32(), 0.0);
        assert_eq!(client.total.to_f32(), 110.0);

        trex.process_transaction(Transaction {
            r#type: TransactionType::Dispute,
            client: 1,
            tx: 2,
            amount: Coins::new(0, 0),
        });
        trex.process_transaction(Transaction {
            r#type: TransactionType::Chargeback,
            client: 1,
            tx: 2,
            amount: Coins::new(0, 0),
        });

        let client = trex.client(1).unwrap();
        assert_eq!(client.available.to_f32(), 100.0);
        assert_eq!(client.held.to_f32(), 0.0);
        assert_eq!(client.total.to_f32(), 100.0);
        assert!(client.locked);
    }

    #[tokio::test]
    async fn test_ignore_invalid_transactions() {
        let mut trex = Trex::default();
        trex.process_transaction(Transaction {
            r#type: TransactionType::Deposit,
            client: 1,
            tx: 1,
            amount: Coins::new(100, 0),
        });
        trex.process_transaction(Transaction {
            r#type: TransactionType::Deposit,
            client: 2,
            tx: 1,
            amount: Coins::new(10, 0),
        });
        trex.process_transaction(Transaction {
            r#type: TransactionType::Dispute,
            client: 2,
            tx: 1,
            amount: Coins::new(0, 0),
        });
        trex.process_transaction(Transaction {
            r#type: TransactionType::Chargeback,
            client: 2,
            tx: 1,
            amount: Coins::new(0, 0),
        });

        let client = trex.client(2).unwrap();
        assert_eq!(client.total.to_f32(), 0.0);
        assert!(client.locked);

        // ignore deposit on locked client
        trex.process_transaction(Transaction {
            r#type: TransactionType::Deposit,
            client: 2,
            tx: 2,
            amount: Coins::new(505, 1),
        });
        assert_eq!(trex.client(2).unwrap().total.to_f32(), 0.0);

        // ignore dispute on non-existing client
        trex.process_transaction(Transaction {
            r#type: TransactionType::Dispute,
            client: 3,
            tx: 2,
            amount: Coins::new(50, 0),
        });
        assert!(trex.client(3).is_none());

        // ignore dispute on non-existing transaction
        trex.process_transaction(Transaction {
            r#type: TransactionType::Dispute,
            client: 3,
            tx: 2,
            amount: Coins::new(50, 0),
        });
        assert!(trex.client(3).is_none());

        // ignore chargeback on transaction without open dispute
        trex.process_transaction(Transaction {
            r#type: TransactionType::Chargeback,
            client: 1,
            tx: 1,
            amount: Coins::new(0, 0),
        });
        assert_eq!(trex.client(1).unwrap().available.to_f32(), 100.0);

        // ignore resolve on transaction without open dispute
        trex.process_transaction(Transaction {
            r#type: TransactionType::Resolve,
            client: 1,
            tx: 1,
            amount: Coins::new(0, 0),
        });
        assert_eq!(trex.client(1).unwrap().available.to_f32(), 100.0);

        // ignore withdrawal of funds under dispute
        trex.process_transaction(Transaction {
            r#type: TransactionType::Dispute,
            client: 1,
            tx: 1,
            amount: Coins::new(0, 0),
        });
        trex.process_transaction(Transaction {
            r#type: TransactionType::Withdrawal,
            client: 1,
            tx: 3,
            amount: Coins::new(10, 0),
        });
        assert_eq!(trex.client(1).unwrap().available.to_f32(), 0.0);
        trex.process_transaction(Transaction {
            r#type: TransactionType::Resolve,
            client: 1,
            tx: 1,
            amount: Coins::new(0, 0),
        });
        assert_eq!(trex.client(1).unwrap().available.to_f32(), 100.0);
    }

    #[tokio::test]
    async fn test_decimals() {
        let mut trex = Trex::default();

        trex.process_transaction(Transaction {
            r#type: TransactionType::Deposit,
            client: 1,
            tx: 1,
            amount: Coins::new(948700, 4),
        });

        let client = trex.client(1).unwrap();
        assert_eq!(client.client, 1);
        assert_eq!(client.available.to_f32(), 94.87);
    }
}
