//! Estimated spend from provider-reported usage. Prices are estimates with a
//! recorded source and retrieval date. Missing usage stays unknown.

use crate::types::Usage;
use serde::Serialize;

#[derive(Debug, Clone, Copy, Serialize)]
pub struct Price {
    pub model_prefix: &'static str,
    pub input_per_mtok: f64,
    pub cached_input_per_mtok: f64,
    pub output_per_mtok: f64,
    pub source: &'static str,
    pub retrieved: &'static str,
}

pub const PRICES: &[Price] = &[
    Price {
        model_prefix: "gpt-5.4-nano",
        input_per_mtok: 0.20,
        cached_input_per_mtok: 0.02,
        output_per_mtok: 1.25,
        source: "https://developers.openai.com/api/docs/pricing (standard tier)",
        retrieved: "2026-10-24",
    },
    Price {
        // Prompts up to 100,000 tokens. Cache writes ($0.125 / MTok for 5 min)
        // are not used by these probes.
        model_prefix: "claude-haiku-5-5",
        input_per_mtok: 0.10,
        cached_input_per_mtok: 0.01,
        output_per_mtok: 0.50,
        source: "https://platform.claude.com/docs/en/about-claude/pricing (Claude Haiku 5.5, prompts up to 100,000 tokens)",
        retrieved: "2026-10-24",
    },
];

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CostEstimate {
    Estimated {
        usd: f64,
        price: Price,
        usage: Usage,
    },
    /// Usage was not reported. `reserved_usd` is the conservative bound
    /// from the request's token limits.
    UsageUnknown { reserved_usd: f64, reason: String },
}

pub fn price_for(model: &str) -> Option<Price> {
    PRICES
        .iter()
        .copied()
        .find(|p| model.starts_with(p.model_prefix))
}

pub fn estimate(
    model: &str,
    usage: Option<&Usage>,
    input_bound: u64,
    output_bound: u64,
) -> CostEstimate {
    let Some(price) = price_for(model) else {
        return CostEstimate::UsageUnknown {
            reserved_usd: f64::NAN,
            reason: format!("no price record for {model}"),
        };
    };
    match usage {
        Some(u) => {
            let fresh = u.input_tokens.saturating_sub(u.cached_input_tokens) as f64;
            let usd = (fresh * price.input_per_mtok
                + u.cached_input_tokens as f64 * price.cached_input_per_mtok
                + u.output_tokens as f64 * price.output_per_mtok)
                / 1e6;
            CostEstimate::Estimated {
                usd,
                price,
                usage: u.clone(),
            }
        }
        None => CostEstimate::UsageUnknown {
            reserved_usd: (input_bound as f64 * price.input_per_mtok
                + output_bound as f64 * price.output_per_mtok)
                / 1e6,
            reason: "provider usage not received".into(),
        },
    }
}

/// Integer micro-dollar cost. 1 token at a price of P dollars per million
/// tokens costs P micro-dollars, so micro-dollars are token counts times the
/// price, rounded up so an estimate never undercounts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum MicroCost {
    Estimated {
        micros: u64,
    },
    /// Usage was not reported: the reservation stays, derived from the bounds.
    UsageUnknown {
        reserved_micros: u64,
        reason: String,
    },
    NoPrice {
        reason: String,
    },
}

/// Prices have at most two decimals, so hundredths of a micro-dollar per token
/// are exact integers.
fn hundredths(price_per_mtok: f64) -> u64 {
    (price_per_mtok * 100.0).round() as u64
}

fn ceil_div(a: u128, b: u128) -> u128 {
    a.div_ceil(b)
}

fn micros_for(price: &Price, fresh_in: u64, cached_in: u64, out: u64) -> u64 {
    let h = fresh_in as u128 * hundredths(price.input_per_mtok) as u128
        + cached_in as u128 * hundredths(price.cached_input_per_mtok) as u128
        + out as u128 * hundredths(price.output_per_mtok) as u128;
    u64::try_from(ceil_div(h, 100)).unwrap_or(u64::MAX)
}

/// Upper bound to reserve before a call: every input token at the full input
/// price and every output token at the output price. `None` without a price.
pub fn reservation_micros(
    model: &str,
    input_tokens_bound: u64,
    output_tokens_bound: u64,
) -> Option<u64> {
    price_for(model).map(|p| micros_for(&p, input_tokens_bound, 0, output_tokens_bound))
}

/// Cost in micro-dollars from reported usage, or the reservation when usage is
/// missing.
pub fn estimate_micros(
    model: &str,
    usage: Option<&Usage>,
    input_bound: u64,
    output_bound: u64,
) -> MicroCost {
    let Some(price) = price_for(model) else {
        return MicroCost::NoPrice {
            reason: format!("no price record for {model}"),
        };
    };
    match usage {
        Some(u) => {
            let cached = u.cached_input_tokens.min(u.input_tokens);
            MicroCost::Estimated {
                micros: micros_for(&price, u.input_tokens - cached, cached, u.output_tokens),
            }
        }
        None => MicroCost::UsageUnknown {
            reserved_micros: micros_for(&price, input_bound, 0, output_bound),
            reason: "provider usage not received".into(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn micros_match_the_recorded_usd_estimate() {
        // anthropic-tool-turn: 508 input and 84 output tokens, recorded as $0.0000928.
        let u = Usage {
            input_tokens: 508,
            cached_input_tokens: 0,
            output_tokens: 84,
            reasoning_tokens: 0,
        };
        assert_eq!(
            estimate_micros("claude-haiku-5-5", Some(&u), 0, 0),
            MicroCost::Estimated { micros: 93 }
        );
        let CostEstimate::Estimated { usd, .. } = estimate("claude-haiku-5-5", Some(&u), 0, 0)
        else {
            panic!()
        };
        assert!((usd * 1e6 - 92.8).abs() < 1e-6);
    }

    #[test]
    fn micros_round_up_and_split_cached_tokens() {
        let u = Usage {
            input_tokens: 1000,
            cached_input_tokens: 400,
            output_tokens: 1,
            reasoning_tokens: 0,
        };
        // 600 * 0.10 + 400 * 0.01 + 1 * 0.50 = 64.5 micro-dollars -> 65
        assert_eq!(
            estimate_micros("claude-haiku-5-5", Some(&u), 0, 0),
            MicroCost::Estimated { micros: 65 }
        );
        let one = Usage {
            input_tokens: 1,
            cached_input_tokens: 0,
            output_tokens: 0,
            reasoning_tokens: 0,
        };
        assert_eq!(
            estimate_micros("claude-haiku-5-5", Some(&one), 0, 0),
            MicroCost::Estimated { micros: 1 }
        );
    }

    #[test]
    fn missing_usage_keeps_the_reservation_and_unknown_models_have_no_price() {
        let r = estimate_micros("claude-haiku-5-5", None, 10_000, 4096);
        // 10000 * 0.10 + 4096 * 0.50 = 3048
        assert_eq!(
            r,
            MicroCost::UsageUnknown {
                reserved_micros: 3048,
                reason: "provider usage not received".into()
            }
        );
        assert_eq!(
            reservation_micros("claude-haiku-5-5", 10_000, 4096),
            Some(3048)
        );
        assert!(matches!(
            estimate_micros("mystery", None, 1, 1),
            MicroCost::NoPrice { .. }
        ));
        assert_eq!(reservation_micros("mystery", 1, 1), None);
    }
}
