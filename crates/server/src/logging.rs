//! Structured JSON logging with redaction.
//!
//! One JSON object per line:
//! `{"timestamp":"...","level":"INFO","target":"http","fields":{"message":"request",...}}`.
//! Fields named after secrets and values that look like credentials are masked by
//! [`crate::redact`] before they reach the writer, whichever crate emitted the event.

use std::fmt;
use std::io::Write;

use serde_json::{Map, Number, Value};
use tracing::field::{Field, Visit};
use tracing::{Event, Subscriber};
use tracing_subscriber::fmt::MakeWriter;
use tracing_subscriber::layer::{Context, SubscriberExt};
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, Layer};

use crate::redact::{REDACTED, is_sensitive_name, scrub_value};

const DEFAULT_FILTER: &str = "info,sqlx=warn";

/// Installs the global subscriber: JSON lines on stdout, filtered by `RUST_LOG`.
pub fn init() {
    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(DEFAULT_FILTER));
    let _ = tracing_subscriber::registry()
        .with(filter)
        .with(JsonLogLayer::new(std::io::stdout))
        .try_init();
}

/// A layer that writes redacted JSON lines to `make_writer`.
pub struct JsonLogLayer<W> {
    make_writer: W,
}

impl<W> JsonLogLayer<W> {
    pub fn new(make_writer: W) -> Self {
        Self { make_writer }
    }
}

impl<S, W> Layer<S> for JsonLogLayer<W>
where
    S: Subscriber,
    W: for<'a> MakeWriter<'a> + 'static,
{
    fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
        let meta = event.metadata();
        let mut visitor = FieldVisitor::default();
        event.record(&mut visitor);
        let mut line = Map::new();
        line.insert(
            "timestamp".into(),
            Value::String(chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)),
        );
        line.insert("level".into(), Value::String(meta.level().to_string()));
        line.insert("target".into(), Value::String(meta.target().to_owned()));
        line.insert("fields".into(), Value::Object(visitor.fields));
        let mut text = Value::Object(line).to_string();
        text.push('\n');
        let mut writer = self.make_writer.make_writer();
        let _ = writer.write_all(text.as_bytes());
    }
}

#[derive(Default)]
struct FieldVisitor {
    fields: Map<String, Value>,
}

impl FieldVisitor {
    fn put(&mut self, field: &Field, value: Value) {
        let value = if is_sensitive_name(field.name()) {
            Value::String(REDACTED.to_owned())
        } else if let Value::String(text) = &value {
            Value::String(scrub_value(text).into_owned())
        } else {
            value
        };
        self.fields.insert(field.name().to_owned(), value);
    }
}

impl Visit for FieldVisitor {
    fn record_debug(&mut self, field: &Field, value: &dyn fmt::Debug) {
        self.put(field, Value::String(format!("{value:?}")));
    }

    fn record_str(&mut self, field: &Field, value: &str) {
        self.put(field, Value::String(value.to_owned()));
    }

    fn record_i64(&mut self, field: &Field, value: i64) {
        self.put(field, Value::Number(value.into()));
    }

    fn record_u64(&mut self, field: &Field, value: u64) {
        self.put(field, Value::Number(value.into()));
    }

    fn record_bool(&mut self, field: &Field, value: bool) {
        self.put(field, Value::Bool(value));
    }

    fn record_f64(&mut self, field: &Field, value: f64) {
        let number = Number::from_f64(value).map_or(Value::Null, Value::Number);
        self.put(field, number);
    }

    fn record_error(&mut self, field: &Field, value: &(dyn std::error::Error + 'static)) {
        self.put(field, Value::String(value.to_string()));
    }
}

#[cfg(test)]
pub(crate) mod capture {
    use std::sync::{Arc, Mutex};

    use tracing_subscriber::fmt::MakeWriter;

    /// A writer that keeps everything in memory.
    #[derive(Clone, Default)]
    pub struct Buffer(pub Arc<Mutex<Vec<u8>>>);

    impl Buffer {
        pub fn text(&self) -> String {
            String::from_utf8_lossy(&self.0.lock().unwrap()).into_owned()
        }
    }

    pub struct Handle(Arc<Mutex<Vec<u8>>>);

    impl std::io::Write for Handle {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(buf);
            Ok(buf.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    impl<'a> MakeWriter<'a> for Buffer {
        type Writer = Handle;

        fn make_writer(&'a self) -> Handle {
            Handle(self.0.clone())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::capture::Buffer;
    use super::*;

    #[test]
    fn secrets_are_masked_whatever_emits_them() {
        let buffer = Buffer::default();
        let subscriber = tracing_subscriber::registry().with(JsonLogLayer::new(buffer.clone()));
        tracing::subscriber::with_default(subscriber, || {
            tracing::info!(password = "hunter2", "sign in");
            tracing::info!(
                cookie = "hf_session=abc123",
                authorization = "Bearer x",
                "headers"
            );
            tracing::info!(
                note = "hf_session=zzz9; Path=/",
                "innocent name, secret value"
            );
            tracing::info!(count = 3, ok = true, path = "/api/v1/projects", "plain");
        });
        let text = buffer.text();
        for secret in ["hunter2", "abc123", "Bearer x", "zzz9"] {
            assert!(!text.contains(secret), "{secret} leaked: {text}");
        }
        assert!(text.contains("[redacted]"));
        let last: Value = serde_json::from_str(text.lines().last().unwrap()).unwrap();
        assert_eq!(last["level"], "INFO");
        assert_eq!(last["fields"]["path"], "/api/v1/projects");
        assert_eq!(last["fields"]["count"], 3);
        assert_eq!(last["fields"]["message"], "plain");
    }
}
