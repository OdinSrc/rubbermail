use std::mem::replace;
use tracing::{trace, debug, warn};

use anyhow::{Context, Result};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Mail {
    pub from: String,
    pub to: Vec<String>,
    pub data: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum State {
    Ready,
    Acknowledged,
    ReceivingRcpt(Mail),
    ReceivingData(Mail),
    Received(Mail),
}

pub struct Connection {
    pub state: State,
    pub ehlo_greeting: String,
}

pub struct SmtpServer {
    connection: Connection,
}

pub const SMTP_READY: &[u8] = b"220 rubbermail\n";
pub const SMTP_OK: &[u8] = b"250 Ok\n";
pub const SMTP_AUTH_OK: &[u8] = b"235 Ok\n";
pub const SMTP_SEND_ME_DATA: &[u8] = b"354 End data with <CR><LF>.<CR><LF>\n";
pub const SMTP_GOODBYE: &[u8] = b"221 Bye\n";
pub const SMTP_EMPTY: &[u8] = &[];

impl Connection {

    pub fn new(domain: impl AsRef<str>) -> Self {
        let domain = domain.as_ref();

        let ehlo_greeting = format!("250-{domain} Hello {domain}\n250 AUTH PLAIN LOGIN\n");

        Self {
            state: State::Ready,
            ehlo_greeting,
        }
    }

    pub fn handle_smtp(&mut self, raw_msg: &str) -> Result<&[u8]> {
        let mut msg = raw_msg.split_whitespace();
        let command = msg.next().context("received empty command")?.to_lowercase();

        // Atomically replace the current state with 'State::Ready' and store the old state in 'state'.
        let state = replace(&mut self.state, State::Ready);

        match (command.as_str(), state) {
            ("ehlo", State::Ready) => {
                trace!("Sending Auth Info");
                self.state = State::Acknowledged;
                Ok(self.ehlo_greeting.as_bytes())
            }
            ("helo", State::Ready) => {
                self.state = State::Acknowledged;
                Ok(SMTP_OK)
            }
            ("noop", _) | ("help", _) | ("info", _) | ("vrfy", _) | ("expn", _) => {
                // Any of this command and in any state
                trace!("Got {command}");
                Ok(SMTP_OK)
            }
            ("rset", _) => {
                self.state = State::Ready;
                Ok(SMTP_OK)
            }
            ("auth", _) => Ok(SMTP_AUTH_OK),
            ("mail", State::Acknowledged) => {
                trace!("Receiving MAIL");
                let from = msg.next().context("received empty MAIL")?;
                let from = from
                    .strip_prefix("FROM:")
                    .context("received incorrect MAIL")?;
                debug!("FROM: {from}");

                self.state = State::ReceivingRcpt(Mail {
                    from: from.to_string(),
                    ..Default::default()
                });

                Ok(SMTP_OK)
            }
            ("rcpt", State::ReceivingRcpt(mut mail)) => {
                trace!("Receiving rcpt");
                let to = msg.next().context("received empty RCPT")?;
                let to = to.strip_prefix("TO:").context("received incorrect RCPT")?;

                debug!("TO: {to}");
                mail.to.push(to.to_string());

                self.state = State::ReceivingRcpt(mail);
                Ok(SMTP_OK)
            }
            ("data", State::ReceivingRcpt(mail)) => {
                trace!("Receiving data");
                self.state = State::ReceivingData(mail);
                Ok(SMTP_SEND_ME_DATA)
            }
            ("quit", State::ReceivingData(mail)) => {
                trace!(
                    "Received data: FROM: {} TO:{} DATA:{}",
                    mail.from,
                    mail.to.join(", "),
                    mail.data
                );
                self.state = State::Received(mail);

                Ok(SMTP_GOODBYE)
            }
            ("quit", _) => {
                warn!("Received quit before getting any data");
                Ok(SMTP_GOODBYE)
            }
            (_, State::ReceivingData(mut mail)) => {
                trace!("Receiving data");
                let resp = if raw_msg.ends_with("\r\n.\r\n") {
                    SMTP_OK
                } else {
                    SMTP_EMPTY
                };

                mail.data += raw_msg;
                self.state = State::ReceivingData(mail);
                Ok(resp)
            }
            _ => anyhow::bail!(
                "Unexpected message received in state {:?}: {raw_msg}",
                self.state
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_smtp_flow() {
        let mut conn = Connection::new("test");
        assert_eq!(conn.state, State::Ready);

        conn.handle_smtp("HELO localhost").unwrap();
        assert_eq!(conn.state, State::Acknowledged);

        conn.handle_smtp("MAIL FROM: <local@example.com>").unwrap();
        assert!(matches!(conn.state, State::ReceivingRcpt(_)));

        conn.handle_smtp("RCPT TO: <receiver@localhost>").unwrap();
        assert!(matches!(conn.state, State::ReceivingRcpt(_)));

        conn.handle_smtp("RCPT TO: <receiver2@localhost>").unwrap();
        assert!(matches!(conn.state, State::ReceivingRcpt(_)));

        conn.handle_smtp("DATA hello world\n").unwrap();
        assert!(matches!(conn.state, State::ReceivingData(_)));

        conn.handle_smtp("DATA hello world2\n").unwrap();
        assert!(matches!(conn.state, State::ReceivingData(_)));

        conn.handle_smtp("QUIT").unwrap();
        assert!(matches!(conn.state, State::Received(_)));
    }

    #[test]
    fn test_no_greeting() {
        let mut sm = Connection::new("test");
        assert_eq!(sm.state, State::Ready);
        for command in [
            "MAIL FROM: <local@example.com>",
            "RCPT TO: <local@example.com>",
            "DATA hey",
            "GARBAGE",
        ] {
            assert!(sm.handle_smtp(command).is_err());
        }
    }
}
