//! The SMTP mailer against a tiny in-process SMTP server (stands in for Mailpit).

use std::sync::{Arc, Mutex};

use factory_accounts::{Mailer, OutgoingMail, SmtpMailer};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;

#[derive(Default, Debug)]
struct Captured {
    commands: Vec<String>,
    data: Vec<String>,
}

async fn fake_smtp() -> (u16, Arc<Mutex<Captured>>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let captured: Arc<Mutex<Captured>> = Arc::default();
    let shared = captured.clone();
    tokio::spawn(async move {
        loop {
            let Ok((stream, _)) = listener.accept().await else {
                return;
            };
            let shared = shared.clone();
            tokio::spawn(async move {
                let (read, mut write) = stream.into_split();
                let mut lines = BufReader::new(read).lines();
                write.write_all(b"220 fake ESMTP\r\n").await.ok();
                let mut in_data = false;
                let mut body = String::new();
                while let Ok(Some(line)) = lines.next_line().await {
                    if in_data {
                        if line == "." {
                            in_data = false;
                            shared.lock().unwrap().data.push(std::mem::take(&mut body));
                            write.write_all(b"250 queued\r\n").await.ok();
                        } else {
                            body.push_str(&line);
                            body.push('\n');
                        }
                        continue;
                    }
                    shared.lock().unwrap().commands.push(line.clone());
                    let upper = line.to_ascii_uppercase();
                    let reply: &[u8] = if upper.starts_with("EHLO") {
                        b"250-fake\r\n250 8BITMIME\r\n"
                    } else if upper.starts_with("DATA") {
                        in_data = true;
                        b"354 go ahead\r\n"
                    } else if upper.starts_with("QUIT") {
                        write.write_all(b"221 bye\r\n").await.ok();
                        return;
                    } else {
                        b"250 ok\r\n"
                    };
                    write.write_all(reply).await.ok();
                }
            });
        }
    });
    (port, captured)
}

#[tokio::test(flavor = "multi_thread")]
async fn smtp_mailer_delivers_to_a_plain_development_server() {
    let (port, captured) = fake_smtp().await;
    let mailer = SmtpMailer::from_url(
        &format!("smtp://127.0.0.1:{port}"),
        "HOI4 Factory <noreply@localhost>",
    )
    .unwrap();
    mailer
        .send(OutgoingMail {
            to: "player@example.org".to_owned(),
            subject: "Confirm your email".to_owned(),
            text: "Open the link to confirm.".to_owned(),
        })
        .await
        .unwrap();
    let got = captured.lock().unwrap();
    assert!(
        got.commands
            .iter()
            .any(|c| c.eq_ignore_ascii_case("MAIL FROM:<noreply@localhost>")),
        "{:?}",
        got.commands
    );
    assert!(
        got.commands
            .iter()
            .any(|c| c.eq_ignore_ascii_case("RCPT TO:<player@example.org>")),
        "{:?}",
        got.commands
    );
    assert_eq!(got.data.len(), 1);
    let message = &got.data[0];
    assert!(message.contains("Subject: Confirm your email"), "{message}");
    assert!(message.contains("To: player@example.org"));
    assert!(message.contains("Open the link to confirm."));
}

#[tokio::test(flavor = "multi_thread")]
async fn smtp_failures_are_reported_as_transport_errors() {
    // Nothing listens on port 1.
    let mailer = SmtpMailer::from_url("smtp://127.0.0.1:1", "noreply@localhost").unwrap();
    let err = mailer
        .send(OutgoingMail {
            to: "player@example.org".to_owned(),
            subject: "s".to_owned(),
            text: "t".to_owned(),
        })
        .await
        .unwrap_err();
    assert!(
        matches!(err, factory_accounts::MailError::Transport(_)),
        "{err}"
    );
    let bad_to = mailer
        .send(OutgoingMail {
            to: "nonsense".to_owned(),
            subject: "s".to_owned(),
            text: "t".to_owned(),
        })
        .await
        .unwrap_err();
    assert!(matches!(bad_to, factory_accounts::MailError::Invalid(_)));
}
