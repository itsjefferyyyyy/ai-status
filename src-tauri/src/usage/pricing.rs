use super::model::UsageEvent;
use std::collections::HashMap;
use std::sync::RwLock;

/// USD price per 1,000,000 tokens. Mirrors the disclaimer in usage/limits.rs:
/// these are reference values for known model families, not fetched from a
/// live pricing API (neither Anthropic nor OpenAI expose one), so they can go
/// stale as providers change pricing. When Claude Code's own log already
/// carries a real `cost_usd` for an event (see agents/claude_code.rs), that
/// exact figure is used instead of this estimate — this table is only the
/// fallback for events with tokens but no first-party cost.
#[derive(Debug, Clone, Copy)]
pub struct ModelPricing {
    pub input_per_million: f64,
    pub output_per_million: f64,
    pub cache_write_per_million: f64,
    pub cache_read_per_million: f64,
}

/// Matched by prefix against a normalized (lowercased) model id, longest
/// prefix first, so e.g. "claude-opus-4-1-20250805" matches "claude-opus-4-1"
/// rather than the shorter "claude-opus-4". Covers the model families known
/// to appear in Claude Code / Codex local logs at time of writing.
const KNOWN_MODELS: &[(&str, ModelPricing)] = &[
    (
        "claude-opus-4-1",
        ModelPricing {
            input_per_million: 15.0,
            output_per_million: 75.0,
            cache_write_per_million: 18.75,
            cache_read_per_million: 1.5,
        },
    ),
    (
        "claude-opus-4",
        ModelPricing {
            input_per_million: 15.0,
            output_per_million: 75.0,
            cache_write_per_million: 18.75,
            cache_read_per_million: 1.5,
        },
    ),
    (
        "claude-sonnet-4",
        ModelPricing {
            input_per_million: 3.0,
            output_per_million: 15.0,
            cache_write_per_million: 3.75,
            cache_read_per_million: 0.3,
        },
    ),
    (
        "claude-3-7-sonnet",
        ModelPricing {
            input_per_million: 3.0,
            output_per_million: 15.0,
            cache_write_per_million: 3.75,
            cache_read_per_million: 0.3,
        },
    ),
    (
        "claude-3-5-sonnet",
        ModelPricing {
            input_per_million: 3.0,
            output_per_million: 15.0,
            cache_write_per_million: 3.75,
            cache_read_per_million: 0.3,
        },
    ),
    (
        "claude-3-5-haiku",
        ModelPricing {
            input_per_million: 0.8,
            output_per_million: 4.0,
            cache_write_per_million: 1.0,
            cache_read_per_million: 0.08,
        },
    ),
    (
        "claude-3-haiku",
        ModelPricing {
            input_per_million: 0.25,
            output_per_million: 1.25,
            cache_write_per_million: 0.3,
            cache_read_per_million: 0.03,
        },
    ),
    (
        "gpt-5",
        ModelPricing {
            input_per_million: 1.25,
            output_per_million: 10.0,
            cache_write_per_million: 1.25,
            cache_read_per_million: 0.125,
        },
    ),
    (
        "gpt-4.1",
        ModelPricing {
            input_per_million: 2.0,
            output_per_million: 8.0,
            cache_write_per_million: 2.0,
            cache_read_per_million: 0.5,
        },
    ),
    (
        "gpt-4o",
        ModelPricing {
            input_per_million: 2.5,
            output_per_million: 10.0,
            cache_write_per_million: 2.5,
            cache_read_per_million: 1.25,
        },
    ),
    (
        "o3",
        ModelPricing {
            input_per_million: 2.0,
            output_per_million: 8.0,
            cache_write_per_million: 2.0,
            cache_read_per_million: 0.5,
        },
    ),
    (
        "o4-mini",
        ModelPricing {
            input_per_million: 1.1,
            output_per_million: 4.4,
            cache_write_per_million: 1.1,
            cache_read_per_million: 0.275,
        },
    ),
    (
        "codex-mini",
        ModelPricing {
            input_per_million: 1.5,
            output_per_million: 6.0,
            cache_write_per_million: 1.5,
            cache_read_per_million: 0.375,
        },
    ),
];

/// Looks up reference pricing for a model id, normalizing case and matching
/// by longest known prefix (model ids commonly carry a trailing date/version
/// suffix, e.g. `claude-opus-4-1-20250805`). Returns `None` for a model not
/// in `KNOWN_MODELS` rather than guessing — callers should treat that as
/// "can't estimate cost for this event", not "cost is zero".
pub fn price_for_model(model: &str) -> Option<ModelPricing> {
    let normalized = model.to_lowercase();
    KNOWN_MODELS
        .iter()
        .filter(|(prefix, _)| normalized.starts_with(prefix))
        .max_by_key(|(prefix, _)| prefix.len())
        .map(|(_, pricing)| *pricing)
}

/// Caches pricing lookups per model id actually observed in local usage logs
/// — rather than eagerly resolving every entry in `KNOWN_MODELS`, entries are
/// added only for models `poll::refresh` has actually seen in a session, so
/// the cache mirrors exactly the set of models the user has locally used.
#[derive(Default)]
pub struct PricingCache {
    resolved: RwLock<HashMap<String, Option<ModelPricing>>>,
}

impl PricingCache {
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns cached pricing for `model`, resolving and caching it first if
    /// this is the first time this model id has been seen.
    pub fn get_or_resolve(&self, model: &str) -> Option<ModelPricing> {
        if let Some(cached) = self.resolved.read().unwrap().get(model) {
            return *cached;
        }
        let resolved = price_for_model(model);
        self.resolved
            .write()
            .unwrap()
            .insert(model.to_string(), resolved);
        resolved
    }
}

/// Sum of estimated API cost across `events`. For an event that already
/// carries a first-party `cost_usd` (Claude Code sometimes logs this
/// directly), that exact value is used; otherwise cost is estimated from the
/// event's token counts and its model's cached price. `incomplete` is true
/// when at least one event contributed nothing to the total because it had
/// no model or an unrecognized one — the caller should surface that instead
/// of presenting the total as exact.
pub struct CostEstimate {
    pub total_usd: f64,
    pub incomplete: bool,
}

pub fn estimate_cost(events: &[UsageEvent], cache: &PricingCache) -> CostEstimate {
    let mut total_usd = 0.0;
    let mut incomplete = false;

    for ev in events {
        if let Some(cost) = ev.cost_usd {
            total_usd += cost;
            continue;
        }
        let Some(model) = &ev.model else {
            incomplete = true;
            continue;
        };
        let Some(price) = cache.get_or_resolve(model) else {
            incomplete = true;
            continue;
        };
        total_usd += ev.tokens.input_tokens as f64 / 1_000_000.0 * price.input_per_million;
        total_usd += ev.tokens.output_tokens as f64 / 1_000_000.0 * price.output_per_million;
        total_usd +=
            ev.tokens.cache_creation_tokens as f64 / 1_000_000.0 * price.cache_write_per_million;
        total_usd +=
            ev.tokens.cache_read_tokens as f64 / 1_000_000.0 * price.cache_read_per_million;
    }

    CostEstimate {
        total_usd,
        incomplete,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_known_model_with_date_suffix() {
        let p = price_for_model("claude-sonnet-4-5-20250929").expect("should match");
        assert_eq!(p.input_per_million, 3.0);
    }

    #[test]
    fn prefers_longest_matching_prefix() {
        let p = price_for_model("claude-opus-4-1-20250805").expect("should match");
        assert_eq!(p.output_per_million, 75.0);
    }

    #[test]
    fn unknown_model_returns_none() {
        assert!(price_for_model("some-future-model-nobody-has-seen").is_none());
    }

    #[test]
    fn cache_resolves_once_and_reuses() {
        let cache = PricingCache::new();
        assert!(cache.get_or_resolve("claude-3-5-haiku-20241022").is_some());
        assert!(cache.get_or_resolve("unknown-model-xyz").is_none());
        // Second lookups should hit the cache and return the same answers.
        assert!(cache.get_or_resolve("claude-3-5-haiku-20241022").is_some());
        assert!(cache.get_or_resolve("unknown-model-xyz").is_none());
    }

    fn event(
        model: Option<&str>,
        cost_usd: Option<f64>,
        input_tokens: u64,
        output_tokens: u64,
    ) -> UsageEvent {
        use super::super::model::TokenTotals;
        use crate::agents::AgentId;
        UsageEvent {
            agent: AgentId::ClaudeCode,
            timestamp: chrono::Utc::now(),
            tokens: TokenTotals {
                input_tokens,
                output_tokens,
                cache_creation_tokens: 0,
                cache_read_tokens: 0,
            },
            cost_usd,
            model: model.map(str::to_string),
            dedup_key: "k".to_string(),
        }
    }

    #[test]
    fn prefers_first_party_cost_over_estimate() {
        let cache = PricingCache::new();
        let events = vec![event(Some("claude-sonnet-4-5-20250929"), Some(9.99), 1_000_000, 0)];
        let result = estimate_cost(&events, &cache);
        assert_eq!(result.total_usd, 9.99);
        assert!(!result.incomplete);
    }

    #[test]
    fn estimates_from_tokens_when_no_first_party_cost() {
        let cache = PricingCache::new();
        let events = vec![event(Some("claude-sonnet-4-5-20250929"), None, 1_000_000, 1_000_000)];
        let result = estimate_cost(&events, &cache);
        assert_eq!(result.total_usd, 3.0 + 15.0);
        assert!(!result.incomplete);
    }

    #[test]
    fn flags_incomplete_for_unknown_or_missing_model() {
        let cache = PricingCache::new();
        let events = vec![
            event(Some("claude-sonnet-4-5-20250929"), None, 1_000_000, 0),
            event(None, None, 500_000, 0),
            event(Some("totally-unknown-model"), None, 500_000, 0),
        ];
        let result = estimate_cost(&events, &cache);
        assert_eq!(result.total_usd, 3.0);
        assert!(result.incomplete);
    }
}
