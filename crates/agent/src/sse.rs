//! Incremental server-sent events parser (WHATWG event-stream format).
//!
//! Bytes arrive in arbitrary chunks. Lines may end in LF, CRLF or CR. A blank
//! line dispatches the pending event. Comment lines start with ':'. Bytes after
//! the last line terminator are kept until more input arrives, so a stream that
//! ends mid-line or mid-event is detectable through [`SseParser::finish`].

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SseEvent {
    pub event: Option<String>,
    pub data: String,
    /// Byte offset in the stream just after the blank line that dispatched it.
    pub end_offset: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SseError {
    #[error("invalid UTF-8 in event stream line at byte {0}")]
    InvalidUtf8(usize),
}

/// What remained unparsed when the byte stream ended.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SseTail {
    /// Bytes after the last line terminator (a partial line).
    pub partial_line_bytes: usize,
    /// A field was read but no blank line dispatched the event.
    pub undispatched_event: bool,
}

impl SseTail {
    pub fn is_clean(&self) -> bool {
        self.partial_line_bytes == 0 && !self.undispatched_event
    }
}

#[derive(Debug, Default)]
pub struct SseParser {
    buf: Vec<u8>,
    consumed: usize,
    pending_cr: bool,
    event: Option<String>,
    data: Vec<String>,
    has_fields: bool,
}

impl SseParser {
    pub fn new() -> Self {
        Self::default()
    }

    /// Total bytes accepted so far.
    pub fn offset(&self) -> usize {
        self.consumed + self.buf.len()
    }

    pub fn push(&mut self, chunk: &[u8]) -> Result<Vec<SseEvent>, SseError> {
        let mut out = Vec::new();
        for &b in chunk {
            if self.pending_cr {
                self.pending_cr = false;
                if b == b'\n' {
                    self.consumed += 1;
                    continue;
                }
            }
            match b {
                b'\n' | b'\r' => {
                    if b == b'\r' {
                        self.pending_cr = true;
                    }
                    let line = std::mem::take(&mut self.buf);
                    let line_start = self.consumed;
                    self.consumed += line.len() + 1;
                    let line =
                        String::from_utf8(line).map_err(|_| SseError::InvalidUtf8(line_start))?;
                    if let Some(ev) = self.line(&line) {
                        out.push(ev);
                    }
                }
                _ => self.buf.push(b),
            }
        }
        Ok(out)
    }

    fn line(&mut self, line: &str) -> Option<SseEvent> {
        if line.is_empty() {
            if !self.has_fields {
                return None;
            }
            self.has_fields = false;
            let data = std::mem::take(&mut self.data).join("\n");
            return Some(SseEvent {
                event: self.event.take(),
                data,
                end_offset: self.consumed,
            });
        }
        if line.starts_with(':') {
            return None;
        }
        let (field, value) = match line.find(':') {
            Some(i) => {
                let v = &line[i + 1..];
                (&line[..i], v.strip_prefix(' ').unwrap_or(v))
            }
            None => (line, ""),
        };
        match field {
            "data" => {
                self.data.push(value.to_owned());
                self.has_fields = true;
            }
            "event" => {
                self.event = Some(value.to_owned());
                self.has_fields = true;
            }
            _ => {}
        }
        None
    }

    /// Report what was left when the transport ended.
    pub fn finish(&self) -> SseTail {
        SseTail {
            partial_line_bytes: self.buf.len(),
            undispatched_event: self.has_fields,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // These tests exercise byte framing only. They carry no model output.
    #[test]
    fn splits_across_chunks_and_line_endings() {
        let mut p = SseParser::new();
        let mut evs = p.push(b"data: a\r").unwrap();
        evs.extend(p.push(b"\n\r\nevent: x\ndata: b\ndata: c\n").unwrap());
        evs.extend(p.push(b"\n: comment\n\n").unwrap());
        assert_eq!(evs.len(), 2);
        assert_eq!(evs[0].data, "a");
        assert_eq!(evs[1].event.as_deref(), Some("x"));
        assert_eq!(evs[1].data, "b\nc");
        assert!(p.finish().is_clean());
    }

    #[test]
    fn reports_partial_tail() {
        let mut p = SseParser::new();
        p.push(b"data: one\n\ndata: tw").unwrap();
        let t = p.finish();
        assert_eq!(t.partial_line_bytes, 8);
        let mut p = SseParser::new();
        p.push(b"data: one\n").unwrap();
        assert!(p.finish().undispatched_event);
    }
}
