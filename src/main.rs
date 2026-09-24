use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    chainwatch_rs::run().await
}
