use anyhow::Result;
use tokio::{io::AsyncWriteExt, net::TcpStream};

use super::protocol::{Connection, SMTP_READY};

pub struct SmtpServer {
    connection: Connection,
    stream: TcpStream,
}

impl SmtpServer {
    pub async fn new(domain: impl AsRef<str>, stream: TcpStream) -> Result<Self> {
        Ok(Self {
            stream,
            connection: Connection::new(domain),
        })
    }

    pub async fn serve(mut self) -> Result<()> {
        self.greet().await?;
        Ok(())
    }

    async fn greet(&mut self) -> Result<()> {
        self.stream
            .write_all(SMTP_READY)
            .await
            .map_err(|e| e.into())
    }
}
