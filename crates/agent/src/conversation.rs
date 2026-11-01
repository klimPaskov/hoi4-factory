//! Provider-neutral loop state.
//!
//! The host persists a [`Conversation`] after every step and restores it to
//! continue, resume after a restart or hand the work to another provider. It
//! holds only text, tool calls and tool results ([`crate::request`]), never
//! provider-private reasoning.
//!
//! Limits are counted here so that every host applies the same ones: a maximum
//! number of model turns, a maximum run of consecutive tool errors, and at
//! most one repair of an invalid structured final result. A limit is exceeded
//! when the count passes the maximum: with a maximum of 3 consecutive tool
//! errors, the fourth stops the loop; with a maximum of 1 structured repair,
//! the second invalid result stops it.

use serde::{Deserialize, Serialize};

use crate::request::{Message, ModelRequest, Part, Role, ToolSpec};
use crate::types::{ToolCall, Turn};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct LoopLimits {
    pub max_turns: u32,
    pub max_consecutive_tool_errors: u32,
    pub max_structured_repairs: u32,
}

impl Default for LoopLimits {
    fn default() -> Self {
        Self {
            max_turns: 20,
            max_consecutive_tool_errors: 3,
            max_structured_repairs: 1,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
#[serde(tag = "stop", rename_all = "snake_case")]
pub enum LoopStop {
    #[error("the turn limit of {limit} was reached")]
    MaxTurns { limit: u32 },
    #[error("more than {limit} consecutive tool errors")]
    ToolErrorLimit { limit: u32 },
    #[error("the structured result stayed invalid after {limit} repair attempt(s)")]
    StructuredRepairLimit { limit: u32 },
    /// The host pushed results that do not match the pending tool calls.
    #[error("invalid loop state: {detail}")]
    InvalidState { detail: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResultKind {
    Ok,
    /// The tool ran and failed, or its arguments were refused.
    ToolError,
    /// The final structured result failed host validation.
    StructuredInvalid,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolResultEntry {
    pub tool_use_id: String,
    pub content: String,
    pub is_error: bool,
    pub kind: ResultKind,
}

impl ToolResultEntry {
    pub fn ok(tool_use_id: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            tool_use_id: tool_use_id.into(),
            content: content.into(),
            is_error: false,
            kind: ResultKind::Ok,
        }
    }
    pub fn tool_error(tool_use_id: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            tool_use_id: tool_use_id.into(),
            content: content.into(),
            is_error: true,
            kind: ResultKind::ToolError,
        }
    }
    /// An invalid structured result; the content is [`repair_message`].
    pub fn structured_invalid(tool_use_id: impl Into<String>, issues: &[String]) -> Self {
        Self {
            tool_use_id: tool_use_id.into(),
            content: repair_message(issues),
            is_error: true,
            kind: ResultKind::StructuredInvalid,
        }
    }
}

/// What the loop should do after a model turn.
#[derive(Debug, Clone, PartialEq)]
pub enum Step {
    ToolCalls(Vec<ToolCall>),
    Finished { text: String },
}

/// The message sent back to the model when its structured final result failed
/// validation. It names every issue and asks for one corrected call.
pub fn repair_message(issues: &[String]) -> String {
    let mut s = String::from(
        "The final result was rejected by validation. Fix every issue below and call the tool again with a corrected result. This is the only repair attempt.",
    );
    for i in issues {
        s.push_str("\n- ");
        s.push_str(i);
    }
    s
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Conversation {
    pub system: String,
    pub messages: Vec<Message>,
    pub turns: u32,
    pub consecutive_tool_errors: u32,
    pub structured_repairs: u32,
    #[serde(default)]
    pub limits: LoopLimits,
    /// Tool-use IDs of the last assistant message that still need results.
    #[serde(default)]
    pending: Vec<String>,
}

impl Conversation {
    pub fn new(system: impl Into<String>, user_text: impl Into<String>) -> Self {
        Self::with_limits(system, user_text, LoopLimits::default())
    }

    pub fn with_limits(
        system: impl Into<String>,
        user_text: impl Into<String>,
        limits: LoopLimits,
    ) -> Self {
        Self {
            system: system.into(),
            messages: vec![Message::user_text(user_text)],
            turns: 0,
            consecutive_tool_errors: 0,
            structured_repairs: 0,
            limits,
            pending: Vec::new(),
        }
    }

    /// Tool calls that still wait for a result.
    pub fn pending_tool_use_ids(&self) -> &[String] {
        &self.pending
    }

    /// Build the next provider-neutral request. Refuses once `max_turns` model
    /// turns were taken.
    pub fn request(
        &self,
        tools: &[ToolSpec],
        max_output_tokens: u32,
    ) -> Result<ModelRequest, LoopStop> {
        if self.turns >= self.limits.max_turns {
            return Err(LoopStop::MaxTurns {
                limit: self.limits.max_turns,
            });
        }
        if !self.pending.is_empty() {
            return Err(LoopStop::InvalidState {
                detail: "tool results are missing for the last assistant message".into(),
            });
        }
        Ok(ModelRequest {
            system: self.system.clone(),
            messages: self.messages.clone(),
            tools: tools.to_vec(),
            max_output_tokens,
        })
    }

    /// Record a completed model turn as an assistant message.
    pub fn push_turn(&mut self, turn: &Turn) -> Step {
        self.turns += 1;
        let mut parts = Vec::new();
        if !turn.text.is_empty() {
            parts.push(Part::text(turn.text.clone()));
        }
        for c in &turn.tool_calls {
            parts.push(Part::ToolUse {
                id: c.id.clone(),
                name: c.name.clone(),
                input: c.arguments.clone(),
            });
        }
        self.messages.push(Message {
            role: Role::Assistant,
            parts,
        });
        if turn.tool_calls.is_empty() {
            self.pending.clear();
            Step::Finished {
                text: turn.text.clone(),
            }
        } else {
            self.pending = turn.tool_calls.iter().map(|c| c.id.clone()).collect();
            Step::ToolCalls(turn.tool_calls.clone())
        }
    }

    /// Add a follow-up user message after a finished turn.
    pub fn push_user_text(&mut self, text: impl Into<String>) {
        self.messages.push(Message::user_text(text));
    }

    /// Record the results of the pending tool calls and apply the limits.
    /// The results are always recorded; the error says why the loop must stop.
    pub fn push_tool_results(&mut self, results: Vec<ToolResultEntry>) -> Result<(), LoopStop> {
        let mut want = self.pending.clone();
        want.sort();
        let mut got: Vec<String> = results.iter().map(|r| r.tool_use_id.clone()).collect();
        got.sort();
        if want != got {
            return Err(LoopStop::InvalidState {
                detail: format!(
                    "expected results for {:?}, got {:?}",
                    self.pending,
                    results.iter().map(|r| &r.tool_use_id).collect::<Vec<_>>()
                ),
            });
        }
        let mut repair_used = false;
        for r in &results {
            match r.kind {
                ResultKind::Ok => self.consecutive_tool_errors = 0,
                ResultKind::ToolError => self.consecutive_tool_errors += 1,
                ResultKind::StructuredInvalid => repair_used = true,
            }
        }
        if repair_used {
            self.structured_repairs += 1;
        }
        self.messages.push(Message {
            role: Role::User,
            parts: results
                .into_iter()
                .map(|r| Part::ToolResult {
                    tool_use_id: r.tool_use_id,
                    content: r.content,
                    is_error: r.is_error,
                })
                .collect(),
        });
        self.pending.clear();
        if self.consecutive_tool_errors > self.limits.max_consecutive_tool_errors {
            return Err(LoopStop::ToolErrorLimit {
                limit: self.limits.max_consecutive_tool_errors,
            });
        }
        if self.structured_repairs > self.limits.max_structured_repairs {
            return Err(LoopStop::StructuredRepairLimit {
                limit: self.limits.max_structured_repairs,
            });
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn turn(text: &str, calls: &[(&str, &str)]) -> Turn {
        Turn {
            provider: "anthropic-messages",
            model: None,
            response_id: None,
            text: text.into(),
            tool_calls: calls
                .iter()
                .enumerate()
                .map(|(i, (id, name))| ToolCall {
                    index: i,
                    id: (*id).into(),
                    name: (*name).into(),
                    raw_arguments: "{}".into(),
                    arguments: json!({}),
                })
                .collect(),
            finish_reason: if calls.is_empty() {
                "end_turn".into()
            } else {
                "tool_use".into()
            },
            usage: None,
            bytes: 0,
            events: 0,
        }
    }

    #[test]
    fn tool_round_trip_builds_neutral_messages() {
        let mut c = Conversation::new("sys", "go");
        assert!(c.request(&[], 100).is_ok());
        let Step::ToolCalls(calls) = c.push_turn(&turn("thinking aloud", &[("t1", "read_file")]))
        else {
            panic!("expected tool calls")
        };
        assert_eq!(calls[0].name, "read_file");
        assert!(c.request(&[], 100).is_err(), "results are pending");
        c.push_tool_results(vec![ToolResultEntry::ok("t1", "contents")])
            .unwrap();
        let req = c.request(&[], 100).unwrap();
        assert_eq!(req.messages.len(), 3);
        assert_eq!(req.messages[1].role, Role::Assistant);
        assert!(matches!(&req.messages[1].parts[1], Part::ToolUse { id, .. } if id == "t1"));
        assert!(
            matches!(&req.messages[2].parts[0], Part::ToolResult { tool_use_id, is_error: false, .. } if tool_use_id == "t1")
        );
        assert_eq!(
            c.push_turn(&turn("done", &[])),
            Step::Finished {
                text: "done".into()
            }
        );
    }

    #[test]
    fn turn_limit_refuses_the_request() {
        let mut c = Conversation::with_limits(
            "s",
            "u",
            LoopLimits {
                max_turns: 2,
                ..Default::default()
            },
        );
        for i in 0..2 {
            assert!(c.request(&[], 10).is_ok());
            let id = format!("t{i}");
            c.push_turn(&turn("", &[(&id, "x")]));
            c.push_tool_results(vec![ToolResultEntry::ok(id, "r")])
                .unwrap();
        }
        assert_eq!(
            c.request(&[], 10).unwrap_err(),
            LoopStop::MaxTurns { limit: 2 }
        );
    }

    #[test]
    fn consecutive_tool_errors_reset_on_success() {
        let mut c = Conversation::new("s", "u");
        for i in 0..3 {
            let id = format!("e{i}");
            c.push_turn(&turn("", &[(&id, "x")]));
            c.push_tool_results(vec![ToolResultEntry::tool_error(id, "bad")])
                .unwrap();
        }
        assert_eq!(c.consecutive_tool_errors, 3);
        c.push_turn(&turn("", &[("ok", "x")]));
        c.push_tool_results(vec![ToolResultEntry::ok("ok", "fine")])
            .unwrap();
        assert_eq!(c.consecutive_tool_errors, 0);
        for i in 0..3 {
            let id = format!("f{i}");
            c.push_turn(&turn("", &[(&id, "x")]));
            c.push_tool_results(vec![ToolResultEntry::tool_error(id, "bad")])
                .unwrap();
        }
        c.push_turn(&turn("", &[("f3", "x")]));
        let e = c
            .push_tool_results(vec![ToolResultEntry::tool_error("f3", "bad")])
            .unwrap_err();
        assert_eq!(e, LoopStop::ToolErrorLimit { limit: 3 });
        // The failing result is still in the history for the audit trail.
        assert!(matches!(
            c.messages.last().unwrap().parts[0],
            Part::ToolResult { .. }
        ));
    }

    #[test]
    fn exactly_one_structured_repair() {
        let mut c = Conversation::new("s", "u");
        c.push_turn(&turn("", &[("fin1", "finish")]));
        c.push_tool_results(vec![ToolResultEntry::structured_invalid(
            "fin1",
            &["/files: required".into(), "/summary: too long".into()],
        )])
        .unwrap();
        assert_eq!(c.structured_repairs, 1);
        let Part::ToolResult {
            content, is_error, ..
        } = &c.messages.last().unwrap().parts[0]
        else {
            panic!()
        };
        assert!(*is_error);
        assert!(content.contains("/files: required") && content.contains("/summary: too long"));
        // The repair is attempted and succeeds: no further stop.
        let mut ok = c.clone();
        ok.push_turn(&turn("", &[("fin2", "finish")]));
        ok.push_tool_results(vec![ToolResultEntry::ok("fin2", "accepted")])
            .unwrap();
        // The repair fails again: stop.
        c.push_turn(&turn("", &[("fin2", "finish")]));
        let e = c
            .push_tool_results(vec![ToolResultEntry::structured_invalid(
                "fin2",
                &["still bad".into()],
            )])
            .unwrap_err();
        assert_eq!(e, LoopStop::StructuredRepairLimit { limit: 1 });
    }

    #[test]
    fn mismatched_results_are_refused_and_state_roundtrips() {
        let mut c = Conversation::new("s", "u");
        c.push_turn(&turn("", &[("a", "x"), ("b", "y")]));
        assert!(matches!(
            c.push_tool_results(vec![ToolResultEntry::ok("a", "r")]),
            Err(LoopStop::InvalidState { .. })
        ));
        assert_eq!(c.pending_tool_use_ids().len(), 2);
        let back: Conversation = serde_json::from_str(&serde_json::to_string(&c).unwrap()).unwrap();
        assert_eq!(back, c);
        c.push_tool_results(vec![
            ToolResultEntry::ok("b", "r"),
            ToolResultEntry::ok("a", "r"),
        ])
        .unwrap();
    }

    #[test]
    fn repair_message_lists_issues() {
        let m = repair_message(&["one".into(), "two".into()]);
        assert!(m.contains("- one") && m.contains("- two"));
    }
}
