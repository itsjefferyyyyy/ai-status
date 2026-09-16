// Mirrors src-tauri/src/agents/mod.rs, usage/model.rs, and settings.rs.
// Keep in sync by hand — there are only a handful of fields.

export type AgentId = "claude_code" | "codex";

export const ALL_AGENTS: AgentId[] = ["claude_code", "codex"];

export const AGENT_LABELS: Record<AgentId, string> = {
  claude_code: "Claude Code",
  codex: "Codex CLI",
};

export type UsageWindowKind = "five_hour" | "daily" | "weekly";

export const ALL_WINDOWS: UsageWindowKind[] = ["five_hour", "daily", "weekly"];

export const WINDOW_LABELS: Record<UsageWindowKind, string> = {
  five_hour: "5-hour",
  daily: "Daily",
  weekly: "Weekly (7d)",
};

export interface TokenTotals {
  input_tokens: number;
  output_tokens: number;
  cache_creation_tokens: number;
  cache_read_tokens: number;
}

export interface UsageSnapshot {
  agent: AgentId;
  window: UsageWindowKind;
  totals: TokenTotals;
  window_start_ms: number;
  window_end_ms: number | null;
  is_active: boolean;
  limit: number | null;
  percent: number | null;
  updated_at_ms: number;
}

export type LimitMode = "none" | "auto" | "custom";

export interface AgentSettings {
  enabled: boolean;
  limit_mode: LimitMode;
  plan_tier: string | null;
  custom_limit: number | null;
}

export interface Settings {
  agents: Record<AgentId, AgentSettings>;
  windows_visible: Record<UsageWindowKind, boolean>;
  primary_agent: AgentId;
}

export function tokenTotal(t: TokenTotals): number {
  return t.input_tokens + t.output_tokens + t.cache_creation_tokens + t.cache_read_tokens;
}
