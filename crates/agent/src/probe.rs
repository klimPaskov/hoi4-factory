//! Capability probing for a model profile.
//!
//! `Listing` does one `GET` of the model list: it spends nothing and tells
//! whether the endpoint is reachable, accepts the credential and lists models.
//! `Inference` additionally sends one tiny streaming request (a closed-schema
//! tool, 64 output tokens) and upgrades text, tool calls, streaming and usage
//! accounting to `Supported` with a `Probe` basis, but only for what it
//! actually observed.
//!
//! Failure rules. A refusal about the account (authentication, permission,
//! quota, rate limit) says nothing about the endpoint's abilities, so the
//! capabilities that could not be observed stay as they were and the classified
//! refusal is recorded. A failure about the endpoint (not found, server error,
//! transport failure, a broken stream) turns the affected capability into
//! `Unknown` or `Unsupported`, never `Supported`. The connection outcome is
//! `Supported` only after a 2xx answer.

use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio::sync::watch;

use crate::destination::DestinationPolicy;
use crate::errors::NormalizedError;
use crate::openai::ApiKey;
use crate::profile::{Capabilities, Capability, CapabilityOutcome, CapabilityState, ModelProfile};
use crate::request::{Message, ModelRequest, ToolSpec};
use crate::types::{HttpClass, HttpRefusal, Partial, TurnError, Usage};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProbeMode {
    /// Model listing only. Spends nothing.
    Listing,
    /// Listing plus one tiny inference request.
    Inference,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProbeReport {
    pub profile_id: String,
    pub connection: CapabilityOutcome,
    pub models: Vec<String>,
    pub capabilities: Capabilities,
    pub refusal: Option<HttpRefusal>,
    pub error: Option<NormalizedError>,
    /// Usage reported by the inference request, for spend records.
    pub usage: Option<Usage>,
    pub notes: Vec<String>,
    pub mode: ProbeMode,
    pub probed_at: String,
}

impl ProbeReport {
    /// Whether the profile's configured model appears in the listing.
    pub fn lists_model(&self, model: &str) -> bool {
        self.models.iter().any(|m| m == model)
    }
}

/// Current UTC time as RFC 3339 with second precision.
pub fn now_rfc3339() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0) as i64;
    let (days, rem) = (secs.div_euclid(86_400), secs.rem_euclid(86_400));
    // Civil date from days since 1970-01-01 (Howard Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z",
        rem / 3600,
        rem % 3600 / 60,
        rem % 60
    )
}

fn account_level(e: &TurnError) -> bool {
    matches!(e, TurnError::Http(h) if matches!(h.class, HttpClass::Auth | HttpClass::Permission | HttpClass::Quota | HttpClass::RateLimited))
}

fn partial_of(e: &TurnError) -> Option<&Partial> {
    match e {
        TurnError::IncompleteOutput { partial, .. }
        | TurnError::Transport { partial, .. }
        | TurnError::MalformedToolArguments { partial, .. }
        | TurnError::MalformedEvent { partial, .. }
        | TurnError::ProviderStreamError { partial, .. }
        | TurnError::Cancelled { partial } => Some(partial),
        TurnError::Http(_) | TurnError::Destination(_) => None,
    }
}

fn connection_for(e: &TurnError, at: &str) -> CapabilityOutcome {
    let state = match e {
        TurnError::Destination(_) => CapabilityState::Unsupported,
        TurnError::Http(h) => match h.class {
            HttpClass::Auth | HttpClass::Permission => CapabilityState::Unsupported,
            _ => CapabilityState::Unknown,
        },
        TurnError::Transport { .. } => match e.normalized().code.as_str() {
            "provider_unreachable" => CapabilityState::Unsupported,
            _ => CapabilityState::Unknown,
        },
        _ => CapabilityState::Unknown,
    };
    CapabilityOutcome::probed(state, format!("{}: {}", e.normalized().code, e.class()), at)
}

fn probe_request() -> ModelRequest {
    ModelRequest {
        system: "You are a connectivity probe. Follow the instruction exactly.".into(),
        messages: vec![Message::user_text(
            "Write the single word ok, then call the probe_echo tool with value \"ok\".",
        )],
        tools: vec![ToolSpec {
            name: "probe_echo".into(),
            description: "Echo a short value. Used only for capability probing.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {"value": {"type": "string", "description": "The value to echo"}},
                "required": ["value"],
                "additionalProperties": false
            }),
        }],
        max_output_tokens: 64,
    }
}

pub async fn probe(
    profile: &ModelProfile,
    key: ApiKey,
    policy: &DestinationPolicy,
    mode: ProbeMode,
) -> ProbeReport {
    let at = now_rfc3339();
    let mut report = ProbeReport {
        profile_id: profile.id.clone(),
        connection: CapabilityOutcome::unknown("not probed"),
        models: Vec::new(),
        capabilities: profile.starting_capabilities(),
        refusal: None,
        error: None,
        usage: None,
        notes: Vec::new(),
        mode,
        probed_at: at.clone(),
    };
    let client = match crate::client::ProviderClient::new(profile.clone(), key, policy) {
        Ok(c) => c,
        Err(r) => {
            let e = TurnError::Destination(r);
            report.connection = connection_for(&e, &at);
            report.error = Some(e.normalized());
            return report;
        }
    };

    let mut reachable = true;
    match client.list_models().await {
        Ok((status, models)) => {
            report.connection = CapabilityOutcome::probed(
                CapabilityState::Supported,
                format!("model listing answered HTTP {status}"),
                &at,
            );
            report.capabilities.set(
                Capability::ModelListing,
                CapabilityOutcome::probed(
                    CapabilityState::Supported,
                    format!("{} models listed", models.len()),
                    &at,
                ),
            );
            report.models = models;
        }
        Err(e) => {
            report.connection = connection_for(&e, &at);
            if let TurnError::Http(h) = &e {
                report.refusal = Some(h.clone());
                if matches!(h.class, HttpClass::NotFound) || matches!(h.status, 405 | 501) {
                    report.capabilities.set(
                        Capability::ModelListing,
                        CapabilityOutcome::probed(
                            CapabilityState::Unsupported,
                            format!("listing answered HTTP {}", h.status),
                            &at,
                        ),
                    );
                }
            }
            if !account_level(&e)
                && !matches!(e, TurnError::Http(ref h) if h.class == HttpClass::NotFound)
            {
                report.capabilities.set(
                    Capability::ModelListing,
                    CapabilityOutcome::probed(CapabilityState::Unknown, e.class(), &at),
                );
            }
            reachable = !matches!(e, TurnError::Destination(_))
                && !matches!(&e, TurnError::Http(h) if matches!(h.class, HttpClass::Auth | HttpClass::Permission))
                && !matches!(e.normalized().code.as_str(), "provider_unreachable");
            report.error = Some(e.normalized());
        }
    }

    if mode == ProbeMode::Inference && reachable {
        let (_keep_open, cancel) = watch::channel(false);
        match client.run_turn(&probe_request(), cancel, None).await {
            Ok(turn) => {
                report.connection = CapabilityOutcome::probed(
                    CapabilityState::Supported,
                    "inference answered HTTP 2xx and completed",
                    &at,
                );
                report.error = None;
                report.refusal = None;
                let sup =
                    |ev: String| CapabilityOutcome::probed(CapabilityState::Supported, ev, &at);
                if turn.events > 1 {
                    report.capabilities.set(
                        Capability::Streaming,
                        sup(format!("{} stream events assembled", turn.events)),
                    );
                }
                if !turn.text.is_empty() {
                    report.capabilities.set(
                        Capability::Text,
                        sup(format!("{} characters of text", turn.text.chars().count())),
                    );
                } else {
                    report.notes.push(
                        "no text was produced before the tool call; text stays as it was".into(),
                    );
                }
                let call_ok = turn.tool_calls.iter().any(|c| {
                    c.name == "probe_echo"
                        && c.arguments.get("value").is_some_and(|v| v.is_string())
                });
                if call_ok {
                    report.capabilities.set(
                        Capability::ToolCalls,
                        sup("probe_echo call assembled with valid arguments".into()),
                    );
                } else {
                    report.notes.push(
                        "the model did not call probe_echo; tool calls stay as they were".into(),
                    );
                }
                match &turn.usage {
                    Some(u) if u.output_tokens > 0 => {
                        report.capabilities.set(
                            Capability::UsageAccounting,
                            sup(format!(
                                "usage reported: {} in, {} out",
                                u.input_tokens, u.output_tokens
                            )),
                        );
                    }
                    _ => report
                        .notes
                        .push("no usage was reported; usage accounting stays as it was".into()),
                }
                report.usage = turn.usage;
            }
            Err(TurnError::Http(h))
                if h.class == HttpClass::InvalidRequest
                    && h.message.as_deref().is_some_and(|m| m.to_ascii_lowercase().contains("tool")) =>
            {
                // The endpoint answered and refused tools for this model (observed with Ollama:
                // "... does not support tools"). That is evidence: tool calls are unsupported.
                // Text and streaming are then measured with one request without tools.
                report.capabilities.set(
                    Capability::ToolCalls,
                    CapabilityOutcome::probed(
                        CapabilityState::Unsupported,
                        format!("tool request refused: {}", h.message.clone().unwrap_or_default()),
                        &at,
                    ),
                );
                report.notes.push("the model refused tools; text was probed without tools".into());
                report.refusal = Some(h);
                let (_keep_open, cancel) = watch::channel(false);
                let mut text_only = probe_request();
                text_only.tools.clear();
                match client.run_turn(&text_only, cancel, None).await {
                    Ok(turn) => {
                        report.error = None;
                        let sup = |ev: String| CapabilityOutcome::probed(CapabilityState::Supported, ev, &at);
                        if turn.events > 1 {
                            report.capabilities.set(Capability::Streaming, sup(format!("{} stream events assembled", turn.events)));
                        }
                        if !turn.text.is_empty() {
                            report.capabilities.set(Capability::Text, sup(format!("{} characters of text", turn.text.chars().count())));
                        }
                        if let Some(u) = turn.usage.as_ref().filter(|u| u.output_tokens > 0) {
                            report.capabilities.set(Capability::UsageAccounting, sup(format!("usage reported: {} in, {} out", u.input_tokens, u.output_tokens)));
                        }
                        report.usage = turn.usage;
                    }
                    Err(e) => {
                        report.notes.push(format!("the text-only request failed: {}", e.class()));
                        report.error = Some(e.normalized());
                    }
                }
            }
            Err(e) => {
                if let TurnError::Http(h) = &e {
                    report.refusal = Some(h.clone());
                }
                // A request-level HTTP refusal is an answer: a connection the listing proved stays
                // proven. Account-level refusals (auth, quota) still mean it cannot be used.
                if !(matches!(e, TurnError::Http(_)) && !account_level(&e) && report.connection.is_supported()) {
                    report.connection = connection_for(&e, &at);
                }
                if !account_level(&e)
                    && !matches!(e, TurnError::Destination(_) | TurnError::Http(_))
                {
                    // The stream itself failed: streaming can no longer be claimed.
                    report.capabilities.set(
                        Capability::Streaming,
                        CapabilityOutcome::probed(CapabilityState::Unknown, e.class(), &at),
                    );
                }
                if let TurnError::Http(h) = &e
                    && !account_level(&e)
                {
                    report
                        .notes
                        .push(format!("inference refused with HTTP {}", h.status));
                }
                if let Some(p) = partial_of(&e) {
                    report.usage = p.usage.clone();
                }
                report.error = Some(e.normalized());
            }
        }
    }
    report
}
