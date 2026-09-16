use super::model::UsageWindowKind;
use crate::agents::AgentId;

/// Built-in, deliberately round-number token ceilings per known subscription
/// tier, used only in "auto-estimate" limit mode. These are community
/// estimates, not values sourced from an official API — neither Anthropic nor
/// OpenAI publish machine-readable quota numbers for their subscription
/// plans. Treat them as rough guidance; a user who knows their real limit
/// should use "custom limit" mode instead (see settings.rs).
pub struct PlanTier {
    pub key: &'static str,
    pub label: &'static str,
    pub five_hour: u64,
    pub daily: u64,
    pub weekly: u64,
}

const CLAUDE_TIERS: &[PlanTier] = &[
    PlanTier {
        key: "claude_pro",
        label: "Claude Pro (approx.)",
        five_hour: 44_000,
        daily: 190_000,
        weekly: 1_300_000,
    },
    PlanTier {
        key: "claude_max_5x",
        label: "Claude Max 5x (approx.)",
        five_hour: 220_000,
        daily: 950_000,
        weekly: 6_500_000,
    },
    PlanTier {
        key: "claude_max_20x",
        label: "Claude Max 20x (approx.)",
        five_hour: 880_000,
        daily: 3_800_000,
        weekly: 26_000_000,
    },
];

const CODEX_TIERS: &[PlanTier] = &[
    PlanTier {
        key: "codex_plus",
        label: "ChatGPT Plus (approx.)",
        five_hour: 40_000,
        daily: 170_000,
        weekly: 1_200_000,
    },
    PlanTier {
        key: "codex_pro",
        label: "ChatGPT Pro (approx.)",
        five_hour: 200_000,
        daily: 850_000,
        weekly: 6_000_000,
    },
];

pub fn plan_tiers_for(agent: AgentId) -> &'static [PlanTier] {
    match agent {
        AgentId::ClaudeCode => CLAUDE_TIERS,
        AgentId::Codex => CODEX_TIERS,
    }
}

pub fn limit_for(agent: AgentId, tier_key: &str, window: UsageWindowKind) -> Option<u64> {
    plan_tiers_for(agent)
        .iter()
        .find(|t| t.key == tier_key)
        .map(|t| match window {
            UsageWindowKind::FiveHour => t.five_hour,
            UsageWindowKind::Daily => t.daily,
            UsageWindowKind::Weekly => t.weekly,
        })
}
