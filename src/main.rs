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
    env_logger::Builder::new()
        .target(env_logger::Target::Stderr)
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
    let mut trex = Trex::default();
    while let Some(record) = records.next().await {
        let Ok(transaction) = record.inspect_err(|err| warn!("invalid transaction {err:#}")) else {
            continue;
        };
        trex.process_transaction(transaction).await;
    }

    let mut writer = AsyncSerializer::from_writer(tokio::io::stdout());
    for client_id in trex.clients().await {
        let Some(client) = trex.client(client_id).await else {
            debug!("client {} does not exist anymore", client_id);
            continue;
        };
        writer.serialize(&client).await?;
    }
    writer.flush().await?;
    Ok(())
}
