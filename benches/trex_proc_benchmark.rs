use criterion::{Criterion, async_executor::FuturesExecutor, criterion_group, criterion_main};
use rand::RngExt;
use trex_proc::trex::Trex;
use trex_proc::{Transaction, TransactionId, TransactionType};

const NUM_CLIENTS: u16 = 500;
const NUM_TRANSACTIONS: TransactionId = 50000;
const DISPUTE_RATE: f64 = 0.01;

fn bench_fuzzy(c: &mut Criterion) {
    c.bench_function("fuzzy", |b| {
        b.to_async(FuturesExecutor).iter(|| async {
            let mut rng = rand::rng();
            let mut pending: Vec<Transaction> = Vec::new();
            let mut trex = Trex::default();
            for tx_id in 0..NUM_TRANSACTIONS {
                let tx = Transaction {
                    client: rng.random_range(1..=NUM_CLIENTS),
                    tx: tx_id,
                    amount: rng.random_range(0.0..=10000.0),
                    r#type: if rng.random_bool(0.52) {
                        TransactionType::Deposit
                    } else {
                        TransactionType::Withdrawal
                    },
                };
                trex.process_transaction(tx.clone());
                if rng.random_bool(DISPUTE_RATE) {
                    pending.push(tx)
                }

                if !pending.is_empty()
                    && rng.random_bool(0.3)
                    && let Some(transaction) = pending.pop()
                {
                    if matches!(
                        transaction.r#type,
                        TransactionType::Deposit | TransactionType::Withdrawal
                    ) {
                        let tx = Transaction {
                            tx: transaction.tx,
                            r#type: TransactionType::Dispute,
                            client: transaction.client,
                            amount: 0.0,
                        };
                        trex.process_transaction(tx.clone());
                        pending.push(tx);
                    } else if matches!(transaction.r#type, TransactionType::Dispute) {
                        trex.process_transaction(Transaction {
                            tx: transaction.tx,
                            r#type: if rng.random_bool(0.5) {
                                TransactionType::Resolve
                            } else {
                                TransactionType::Chargeback
                            },
                            client: transaction.client,
                            amount: 0.0,
                        });
                    }
                }
            }

            while let Some(transaction) = pending.pop() {
                if matches!(transaction.r#type, TransactionType::Dispute) {
                    trex.process_transaction(Transaction {
                        tx: transaction.tx,
                        r#type: if rng.random_bool(0.5) {
                            TransactionType::Resolve
                        } else {
                            TransactionType::Chargeback
                        },
                        client: transaction.client,
                        amount: 0.0,
                    });
                }
            }
        });
    });
}

criterion_group!(benches, bench_fuzzy,);
criterion_main!(benches);
