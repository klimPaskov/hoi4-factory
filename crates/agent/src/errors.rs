//! Normalized failures for the contract envelope.
//!
//! Every [`TurnError`] maps to a stable code, one of the five retry classes of
//! the common envelope and a statement of whether the provider may have charged.
//!
//! The rule behind the table: a request that reached the provider, or may have,
//! is never repeated automatically. The host keeps its spend reservation and
//! reconciles first. Only a request known not to have been accepted, or one the
//! provider refused with a transient status before doing any work, is
//! `safe_same_inputs`. A quota refusal is terminal because retrying cannot
//! succeed until a person changes the account.

use serde::{Deserialize, Serialize};

use crate::profile::CapabilityUnsatisfied;
use crate::transport::TransportError;
use crate::types::{Acceptance, HttpClass, TurnError};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RetryClass {
    SafeSameInputs,
    RefreshPreconditions,
    ReconcileFirst,
    RequiresAuthorization,
    Terminal,
}

/// Whether the provider may have charged for the request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CostCertainty {
    /// The provider refused or never received the request.
    None,
    /// The request may have been processed. Keep the reservation.
    Possible,
    /// The provider processed the request to the end of its output.
    Incurred,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NormalizedError {
    pub code: String,
    pub retry_class: RetryClass,
    pub message: String,
    pub provider_status: Option<u16>,
    pub cost: CostCertainty,
}

impl NormalizedError {
    fn new(
        code: &str,
        retry_class: RetryClass,
        cost: CostCertainty,
        message: String,
        provider_status: Option<u16>,
    ) -> Self {
        Self {
            code: code.to_owned(),
            retry_class,
            message,
            provider_status,
            cost,
        }
    }

    /// A required capability is missing or unknown for the profile.
    pub fn capability_unsatisfied(c: &CapabilityUnsatisfied) -> Self {
        Self::new(
            "capability_unsatisfied",
            RetryClass::Terminal,
            CostCertainty::None,
            c.to_string(),
            None,
        )
    }
}

pub fn normalize(e: &TurnError) -> NormalizedError {
    use CostCertainty as C;
    use RetryClass as R;
    let msg = e.to_string();
    match e {
        TurnError::Transport {
            error, acceptance, ..
        } => match (error, acceptance) {
            (TransportError::ResponseTooLarge(_), _) => NormalizedError::new(
                "provider_response_too_large",
                R::Terminal,
                C::Possible,
                msg,
                None,
            ),
            (TransportError::Recording(_), _) => {
                NormalizedError::new("recording_failed", R::Terminal, C::Possible, msg, None)
            }
            (_, Acceptance::NotAccepted) => NormalizedError::new(
                "provider_unreachable",
                R::SafeSameInputs,
                C::None,
                msg,
                None,
            ),
            (_, Acceptance::Accepted | Acceptance::Uncertain) => NormalizedError::new(
                "provider_transport_uncertain",
                R::ReconcileFirst,
                C::Possible,
                msg,
                None,
            ),
        },
        TurnError::IncompleteOutput { .. } => NormalizedError::new(
            "provider_output_incomplete",
            R::ReconcileFirst,
            C::Possible,
            msg,
            None,
        ),
        TurnError::Cancelled { .. } => {
            NormalizedError::new("cancelled", R::ReconcileFirst, C::Possible, msg, None)
        }
        TurnError::ProviderStreamError { .. } => NormalizedError::new(
            "provider_stream_error",
            R::ReconcileFirst,
            C::Possible,
            msg,
            None,
        ),
        // The whole response arrived, so it was processed and billed. The host
        // may report the problem back to the model as a tool error.
        TurnError::MalformedToolArguments { .. } => NormalizedError::new(
            "malformed_tool_arguments",
            R::Terminal,
            C::Incurred,
            msg,
            None,
        ),
        TurnError::MalformedEvent { .. } => NormalizedError::new(
            "provider_event_malformed",
            R::Terminal,
            C::Possible,
            msg,
            None,
        ),
        TurnError::Destination(_) => {
            NormalizedError::new("destination_blocked", R::Terminal, C::None, msg, None)
        }
        TurnError::Http(h) => {
            let s = Some(h.status);
            match h.class {
                HttpClass::Auth => NormalizedError::new(
                    "provider_auth_failed",
                    R::RequiresAuthorization,
                    C::None,
                    msg,
                    s,
                ),
                HttpClass::Permission => NormalizedError::new(
                    "provider_permission_denied",
                    R::RequiresAuthorization,
                    C::None,
                    msg,
                    s,
                ),
                HttpClass::Quota => {
                    NormalizedError::new("provider_quota_exhausted", R::Terminal, C::None, msg, s)
                }
                HttpClass::RateLimited => NormalizedError::new(
                    "provider_rate_limited",
                    R::SafeSameInputs,
                    C::None,
                    msg,
                    s,
                ),
                HttpClass::Overloaded => {
                    NormalizedError::new("provider_overloaded", R::SafeSameInputs, C::None, msg, s)
                }
                HttpClass::Server => NormalizedError::new(
                    "provider_server_error",
                    R::SafeSameInputs,
                    C::None,
                    msg,
                    s,
                ),
                HttpClass::InvalidRequest => {
                    NormalizedError::new("provider_invalid_request", R::Terminal, C::None, msg, s)
                }
                HttpClass::NotFound => {
                    NormalizedError::new("provider_not_found", R::Terminal, C::None, msg, s)
                }
                HttpClass::Other => {
                    NormalizedError::new("provider_refused", R::Terminal, C::None, msg, s)
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::destination::DestinationRefusal;
    use crate::types::{HttpRefusal, Partial};

    fn http(status: u16, class: HttpClass) -> TurnError {
        TurnError::Http(HttpRefusal {
            status,
            class,
            provider_code: None,
            provider_type: None,
            message: None,
        })
    }
    fn transport(error: TransportError, acceptance: Acceptance) -> TurnError {
        TurnError::Transport {
            error,
            acceptance,
            partial: Partial::default(),
        }
    }

    /// The table of the contract: code, retry class, cost, automatic retry.
    #[test]
    fn normalization_table() {
        use CostCertainty as C;
        use RetryClass as R;
        let p = || Partial::default();
        let rows: Vec<(TurnError, &str, R, C, bool)> = vec![
            (
                transport(
                    TransportError::ConnectFailed("x".into()),
                    Acceptance::NotAccepted,
                ),
                "provider_unreachable",
                R::SafeSameInputs,
                C::None,
                true,
            ),
            (
                transport(TransportError::AfterSend("x".into()), Acceptance::Uncertain),
                "provider_transport_uncertain",
                R::ReconcileFirst,
                C::Possible,
                false,
            ),
            (
                transport(TransportError::BodyBroken("x".into()), Acceptance::Accepted),
                "provider_transport_uncertain",
                R::ReconcileFirst,
                C::Possible,
                false,
            ),
            (
                transport(TransportError::ResponseTooLarge(8), Acceptance::Accepted),
                "provider_response_too_large",
                R::Terminal,
                C::Possible,
                false,
            ),
            (
                TurnError::IncompleteOutput {
                    detail: "d".into(),
                    partial: p(),
                },
                "provider_output_incomplete",
                R::ReconcileFirst,
                C::Possible,
                false,
            ),
            (
                TurnError::Cancelled { partial: p() },
                "cancelled",
                R::ReconcileFirst,
                C::Possible,
                false,
            ),
            (
                TurnError::ProviderStreamError {
                    detail: "d".into(),
                    partial: p(),
                },
                "provider_stream_error",
                R::ReconcileFirst,
                C::Possible,
                false,
            ),
            (
                TurnError::MalformedToolArguments {
                    index: 0,
                    detail: "d".into(),
                    partial: p(),
                },
                "malformed_tool_arguments",
                R::Terminal,
                C::Incurred,
                false,
            ),
            (
                TurnError::MalformedEvent {
                    detail: "d".into(),
                    partial: p(),
                },
                "provider_event_malformed",
                R::Terminal,
                C::Possible,
                false,
            ),
            (
                TurnError::Destination(DestinationRefusal::new("blocked_address", "x")),
                "destination_blocked",
                R::Terminal,
                C::None,
                false,
            ),
            (
                http(401, HttpClass::Auth),
                "provider_auth_failed",
                R::RequiresAuthorization,
                C::None,
                false,
            ),
            (
                http(403, HttpClass::Permission),
                "provider_permission_denied",
                R::RequiresAuthorization,
                C::None,
                false,
            ),
            (
                http(429, HttpClass::Quota),
                "provider_quota_exhausted",
                R::Terminal,
                C::None,
                false,
            ),
            (
                http(429, HttpClass::RateLimited),
                "provider_rate_limited",
                R::SafeSameInputs,
                C::None,
                true,
            ),
            (
                http(529, HttpClass::Overloaded),
                "provider_overloaded",
                R::SafeSameInputs,
                C::None,
                true,
            ),
            (
                http(500, HttpClass::Server),
                "provider_server_error",
                R::SafeSameInputs,
                C::None,
                true,
            ),
            (
                http(400, HttpClass::InvalidRequest),
                "provider_invalid_request",
                R::Terminal,
                C::None,
                false,
            ),
            (
                http(404, HttpClass::NotFound),
                "provider_not_found",
                R::Terminal,
                C::None,
                false,
            ),
            (
                http(418, HttpClass::Other),
                "provider_refused",
                R::Terminal,
                C::None,
                false,
            ),
        ];
        for (err, code, class, cost, auto) in rows {
            let n = err.normalized();
            assert_eq!(
                (n.code.as_str(), n.retry_class, n.cost),
                (code, class, cost),
                "{err}"
            );
            assert_eq!(err.auto_retry_allowed(), auto, "{err}");
            assert_eq!(auto, class == R::SafeSameInputs);
        }
    }

    #[test]
    fn provider_status_is_carried_and_quota_is_never_retryable() {
        let n = http(429, HttpClass::Quota).normalized();
        assert_eq!(n.provider_status, Some(429));
        assert_ne!(n.retry_class, RetryClass::SafeSameInputs);
    }

    #[test]
    fn envelope_names_are_snake_case() {
        assert_eq!(
            serde_json::to_string(&RetryClass::SafeSameInputs).unwrap(),
            "\"safe_same_inputs\""
        );
        assert_eq!(
            serde_json::to_string(&RetryClass::RequiresAuthorization).unwrap(),
            "\"requires_authorization\""
        );
        assert_eq!(
            serde_json::to_string(&CostCertainty::Possible).unwrap(),
            "\"possible\""
        );
    }
}
