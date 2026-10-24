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
    Estimated { usd: f64, price: Price, usage: Usage },
    /// Usage was not reported. `reserved_usd` is the conservative bound
    /// from the request's token limits.
    UsageUnknown { reserved_usd: f64, reason: String },
}

pub fn price_for(model: &str) -> Option<Price> {
    PRICES.iter().copied().find(|p| model.starts_with(p.model_prefix))
}

pub fn estimate(model: &str, usage: Option<&Usage>, input_bound: u64, output_bound: u64) -> CostEstimate {
    let Some(price) = price_for(model) else {
        return CostEstimate::UsageUnknown { reserved_usd: f64::NAN, reason: format!("no price record for {model}") };
    };
    match usage {
        Some(u) => {
            let fresh = u.input_tokens.saturating_sub(u.cached_input_tokens) as f64;
            let usd = (fresh * price.input_per_mtok
                + u.cached_input_tokens as f64 * price.cached_input_per_mtok
                + u.output_tokens as f64 * price.output_per_mtok)
                / 1e6;
            CostEstimate::Estimated { usd, price, usage: u.clone() }
        }
        None => CostEstimate::UsageUnknown {
            reserved_usd: (input_bound as f64 * price.input_per_mtok + output_bound as f64 * price.output_per_mtok) / 1e6,
            reason: "provider usage not received".into(),
        },
    }
}
