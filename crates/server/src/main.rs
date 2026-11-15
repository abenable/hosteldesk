//! HostelDesk sync server. Requires `DATABASE_URL` and listens on
//! `HOSTELDESK_BIND` (default `0.0.0.0:8080`).

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let database_url =
        std::env::var("DATABASE_URL").map_err(|_| anyhow::anyhow!("DATABASE_URL is required"))?;
    let addr = std::env::var("HOSTELDESK_BIND").unwrap_or_else(|_| "0.0.0.0:8080".to_string());
    tracing::info!(%addr, "listening");
    server::serve(&addr, &database_url).await
}
