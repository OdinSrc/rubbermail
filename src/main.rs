use std::env;

use anyhow::Result;
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;
mod smtp;
use tokio::net::TcpListener;

use crate::smtp::server::SmtpServer;

#[tokio::main]
async fn main() -> Result<()> {
    // setting up logging
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber)
        .expect("setting default subscriber failed");

    info!("Starting SMTP Server");
    start_server().await?;
    Ok(())
}

async fn start_server() -> Result<()> {
        let addr = env::args()
        .nth(1)
        .unwrap_or_else(|| "0.0.0.0:2525".to_string());

        let domain = &env::args()
        .nth(2)
        .unwrap_or_else(|| "smtp.localhost".to_string());


        let listener = TcpListener::bind(&addr).await?;
        tracing::info!("Listening on: {}", addr);
        
        loop {
            let (stream, addr) = listener.accept().await?;
            tracing::info!("Accepted a connection from {}", addr);

            tokio::task::LocalSet::new()
            .run_until(async move {
                let smtp = SmtpServer::new(domain, stream).await?;
                smtp.serve().await
            })
            .await
            .ok();

        }

}
