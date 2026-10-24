//! Outgoing mail behind a trait: an SMTP implementation (Mailpit in development) and a test double.

use std::sync::Mutex;

use async_trait::async_trait;
use lettre::message::Mailbox;
use lettre::{AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutgoingMail {
    pub to: String,
    pub subject: String,
    pub text: String,
}

#[derive(Debug, thiserror::Error)]
pub enum MailError {
    #[error("invalid mail configuration or message: {0}")]
    Invalid(String),
    #[error("mail transport failed: {0}")]
    Transport(String),
}

#[async_trait]
pub trait Mailer: Send + Sync + 'static {
    async fn send(&self, mail: OutgoingMail) -> Result<(), MailError>;
}

/// SMTP delivery through lettre.
pub struct SmtpMailer {
    transport: AsyncSmtpTransport<Tokio1Executor>,
    from: Mailbox,
}

impl SmtpMailer {
    /// `url` is a lettre SMTP URL such as `smtp://mailpit:1025` (plain, for development) or
    /// `smtps://user:password@mail.example:465`. `from` is `Name <address>` or a bare address.
    pub fn from_url(url: &str, from: &str) -> Result<Self, MailError> {
        let transport = AsyncSmtpTransport::<Tokio1Executor>::from_url(url)
            .map_err(|e| MailError::Invalid(e.to_string()))?
            .build();
        let from = from
            .parse::<Mailbox>()
            .map_err(|e| MailError::Invalid(format!("from address: {e}")))?;
        Ok(Self { transport, from })
    }
}

pub(crate) fn build_message(from: &Mailbox, mail: &OutgoingMail) -> Result<Message, MailError> {
    let to = mail
        .to
        .parse::<Mailbox>()
        .map_err(|e| MailError::Invalid(format!("recipient: {e}")))?;
    Message::builder()
        .from(from.clone())
        .to(to)
        .subject(mail.subject.clone())
        .header(lettre::message::header::ContentType::TEXT_PLAIN)
        .body(mail.text.clone())
        .map_err(|e| MailError::Invalid(e.to_string()))
}

#[async_trait]
impl Mailer for SmtpMailer {
    async fn send(&self, mail: OutgoingMail) -> Result<(), MailError> {
        let message = build_message(&self.from, &mail)?;
        self.transport
            .send(message)
            .await
            .map(|_| ())
            .map_err(|e| MailError::Transport(e.to_string()))
    }
}

/// Collects mail in memory. Can be told to fail, to exercise the failure path.
#[derive(Default)]
pub struct MemoryMailer {
    sent: Mutex<Vec<OutgoingMail>>,
    fail: Mutex<bool>,
}

impl MemoryMailer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn sent(&self) -> Vec<OutgoingMail> {
        self.sent.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    /// While set, every send fails with a transport error.
    pub fn set_failing(&self, failing: bool) {
        *self.fail.lock().unwrap_or_else(|e| e.into_inner()) = failing;
    }

    /// The most recent mail to `to` (compared ignoring case), if any.
    pub fn last_to(&self, to: &str) -> Option<OutgoingMail> {
        self.sent()
            .into_iter()
            .rev()
            .find(|m| m.to.eq_ignore_ascii_case(to))
    }
}

#[async_trait]
impl Mailer for MemoryMailer {
    async fn send(&self, mail: OutgoingMail) -> Result<(), MailError> {
        if *self.fail.lock().unwrap_or_else(|e| e.into_inner()) {
            return Err(MailError::Transport("simulated failure".to_owned()));
        }
        self.sent
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push(mail);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn message_has_plain_text_body_and_addresses() {
        let from: Mailbox = "HOI4 Factory <noreply@localhost>".parse().unwrap();
        let mail = OutgoingMail {
            to: "player@example.org".to_owned(),
            subject: "Verify".to_owned(),
            text: "Open the link".to_owned(),
        };
        let raw = String::from_utf8(build_message(&from, &mail).unwrap().formatted()).unwrap();
        assert!(raw.contains("To: player@example.org"));
        let from_line = raw.lines().find(|l| l.starts_with("From:")).unwrap();
        assert!(
            from_line.contains("HOI4 Factory") && from_line.contains("<noreply@localhost>"),
            "{from_line}"
        );
        assert!(raw.contains("Subject: Verify"));
        assert!(raw.contains("Open the link"));
    }

    #[test]
    fn bad_recipient_and_bad_url_are_rejected() {
        let from: Mailbox = "noreply@localhost".parse().unwrap();
        let bad = OutgoingMail {
            to: "not an address".to_owned(),
            subject: "s".to_owned(),
            text: "t".to_owned(),
        };
        assert!(build_message(&from, &bad).is_err());
        assert!(SmtpMailer::from_url("not a url", "noreply@localhost").is_err());
        assert!(SmtpMailer::from_url("smtp://mailpit:1025", "bad from").is_err());
        assert!(
            SmtpMailer::from_url("smtp://mailpit:1025", "HOI4 Factory <noreply@localhost>").is_ok()
        );
    }

    #[tokio::test]
    async fn memory_mailer_collects_and_can_fail() {
        let m = MemoryMailer::new();
        let mail = OutgoingMail {
            to: "a@example.org".to_owned(),
            subject: "s".to_owned(),
            text: "t".to_owned(),
        };
        m.send(mail.clone()).await.unwrap();
        assert_eq!(m.last_to("a@example.org"), Some(mail.clone()));
        m.set_failing(true);
        assert!(m.send(mail).await.is_err());
        assert_eq!(m.sent().len(), 1);
    }
}
