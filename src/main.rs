use csv_async::AsyncSerializer;
use eyre::{Context, eyre};
use log::{debug, warn};
use std::env;
use std::path::PathBuf;
use tokio_stream::StreamExt;
use trex_proc::Transaction;
use trex_proc::trex::Trex;

#[tokio::main]
async fn main() -> eyre::Result<()> {
    // For this simple app `env_logger` seems to be enough.
    // Should be trivial to switch to `tracing` framework when needed.
    env_logger::Builder::new()
        .target(env_logger::Target::Stderr) // should not pollute stdout with logs
        .init();

    let mut args = env::args();
    let input_path = PathBuf::from(
        args.nth(1)
            .ok_or_else(|| eyre!("CSV input path not provided"))?,
    );
    let in_file = tokio::fs::File::open(&input_path)
        .await
        .with_context(|| format!("failed to read input csv {}", input_path.display()))?;
    let mut rdr = csv_async::AsyncReaderBuilder::new()
        .trim(csv_async::Trim::All)
        .create_deserializer(in_file);
    let mut records = rdr.deserialize::<Transaction>();

    // For this simple app single Trex instance is enough.
    // In more demanding context multiple Trex instances could be created, to handle different groups of clients.
    // Client transactions would be then distributed by simple modulo operation on client ID or Rendezvous hashing
    // if number of trexes is expected to change.
    // Each trex instance could run in separate task and transactions would be sent e.g. using mpsc channel.
    let mut trex = Trex::default();
    while let Some(record) = records.next().await {
        let Ok(transaction) = record.inspect_err(|err| warn!("invalid transaction {err:#}")) else {
            continue;
        };
        trex.process_transaction(transaction);
    }

    let mut writer = AsyncSerializer::from_writer(tokio::io::stdout());
    for client_id in trex.clients() {
        let Some(client) = trex.client(client_id) else {
            debug!("client {} does not exist anymore", client_id);
            continue;
        };
        writer.serialize(&client).await?;
    }
    writer.flush().await?;
    Ok(())
}
