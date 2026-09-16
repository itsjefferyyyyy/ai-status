<script lang="ts">
  import { onMount } from "svelte";
  import { getSettings, updateSettings, listPlanTiers, type PlanTierOption } from "./settings";
  import { ALL_AGENTS, ALL_WINDOWS, AGENT_LABELS, WINDOW_LABELS, type AgentId, type LimitMode, type Settings } from "./types";

  let settings = $state<Settings | null>(null);
  let tiersByAgent = $state<Record<string, PlanTierOption[]>>({});

  onMount(async () => {
    settings = await getSettings();
    for (const agent of ALL_AGENTS) {
      tiersByAgent[agent] = await listPlanTiers(agent);
    }
  });

  async function persist() {
    if (settings) await updateSettings(settings);
  }

  function setAgentEnabled(agent: AgentId, enabled: boolean) {
    if (!settings) return;
    settings.agents[agent].enabled = enabled;
    persist();
  }

  function setLimitMode(agent: AgentId, mode: LimitMode) {
    if (!settings) return;
    settings.agents[agent].limit_mode = mode;
    persist();
  }

  function setPlanTier(agent: AgentId, tier: string) {
    if (!settings) return;
    settings.agents[agent].plan_tier = tier;
    persist();
  }

  function setCustomLimit(agent: AgentId, value: string) {
    if (!settings) return;
    const n = Number(value);
    settings.agents[agent].custom_limit = Number.isFinite(n) && n > 0 ? n : null;
    persist();
  }

  function setWindowVisible(window: string, visible: boolean) {
    if (!settings) return;
    settings.windows_visible[window as keyof Settings["windows_visible"]] = visible;
    persist();
  }

  function setPrimaryAgent(agent: AgentId) {
    if (!settings) return;
    settings.primary_agent = agent;
    persist();
  }

  function setFeeCalculatorEnabled(enabled: boolean) {
    if (!settings) return;
    settings.fee_calculator_enabled = enabled;
    persist();
  }
</script>

<main class="settings">
  <h1>AI Status Settings</h1>

  {#if settings}
    <section>
      <h2>Agents</h2>
      {#each ALL_AGENTS as agent (agent)}
        {@const agentSettings = settings.agents[agent]}
        <div class="agent-block">
          <label class="row">
            <input
              type="checkbox"
              checked={agentSettings.enabled}
              onchange={(e) => setAgentEnabled(agent, e.currentTarget.checked)}
            />
            {AGENT_LABELS[agent]}
          </label>

          {#if agentSettings.enabled}
            <div class="limit-mode">
              <span class="hint">Percentage estimate:</span>
              <select value={agentSettings.limit_mode} onchange={(e) => setLimitMode(agent, e.currentTarget.value as LimitMode)}>
                <option value="none">None (tokens only)</option>
                <option value="auto">Auto-estimate</option>
                <option value="custom">My own limit</option>
              </select>

              {#if agentSettings.limit_mode === "auto"}
                <select
                  value={agentSettings.plan_tier ?? ""}
                  onchange={(e) => setPlanTier(agent, e.currentTarget.value)}
                >
                  <option value="" disabled>Choose a plan…</option>
                  {#each tiersByAgent[agent] ?? [] as tier (tier.key)}
                    <option value={tier.key}>{tier.label}</option>
                  {/each}
                </select>
                <p class="disclaimer">Unofficial estimate — not sourced from an official quota API.</p>
              {:else if agentSettings.limit_mode === "custom"}
                <input
                  type="number"
                  min="0"
                  placeholder="Token limit (5-hour window)"
                  value={agentSettings.custom_limit ?? ""}
                  onchange={(e) => setCustomLimit(agent, e.currentTarget.value)}
                />
              {/if}
            </div>
          {/if}
        </div>
      {/each}
    </section>

    <section>
      <h2>Usage windows</h2>
      {#each ALL_WINDOWS as window (window)}
        <label class="row">
          <input
            type="checkbox"
            checked={settings.windows_visible[window]}
            onchange={(e) => setWindowVisible(window, e.currentTarget.checked)}
          />
          {WINDOW_LABELS[window]}
        </label>
      {/each}
    </section>

    <section>
      <h2>Toolbar</h2>
      <p class="hint">Shown next to the tray icon (5-hour window, regardless of the windows above):</p>
      {#each ALL_AGENTS as agent (agent)}
        <label class="row">
          <input
            type="radio"
            name="primary-agent"
            checked={settings.primary_agent === agent}
            onchange={() => setPrimaryAgent(agent)}
          />
          {agent === "claude_code" ? "C" : "X"} — {AGENT_LABELS[agent]}
        </label>
      {/each}
      {#if !settings.agents[settings.primary_agent]?.enabled}
        <p class="disclaimer">This agent is disabled above, so the toolbar will be blank until you enable it.</p>
      {/if}
    </section>

    <section>
      <h2>API fee calculator</h2>
      <label class="row">
        <input
          type="checkbox"
          checked={settings.fee_calculator_enabled}
          onchange={(e) => setFeeCalculatorEnabled(e.currentTarget.checked)}
        />
        Estimate API cost alongside token counts
      </label>
      <p class="disclaimer">
        Only priced for models seen in your local logs; uses each log's own reported cost when
        available, otherwise a reference price table that can go stale as providers change pricing.
      </p>
    </section>
  {:else}
    <p>Loading…</p>
  {/if}
</main>

<style>
  .settings {
    font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
    padding: 16px 20px;
    font-size: 13px;
    color: #1a1a1a;
    background: #f6f6f6;
    min-height: 100vh;
    box-sizing: border-box;
  }

  h1 {
    font-size: 15px;
    margin: 0 0 14px 0;
  }

  h2 {
    font-size: 12px;
    text-transform: uppercase;
    letter-spacing: 0.03em;
    color: #555;
    margin: 0 0 8px 0;
  }

  section {
    margin-bottom: 18px;
  }

  .row {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 4px 0;
  }

  .agent-block {
    padding: 6px 0;
    border-bottom: 1px solid rgba(0, 0, 0, 0.06);
  }

  .limit-mode {
    display: flex;
    flex-direction: column;
    gap: 6px;
    margin: 6px 0 4px 24px;
  }

  .hint {
    color: #666;
    font-size: 11px;
  }

  select,
  input[type="number"] {
    font-size: 12px;
    padding: 4px 6px;
    border-radius: 6px;
    border: 1px solid rgba(0, 0, 0, 0.15);
  }

  .disclaimer {
    margin: 0;
    font-size: 10px;
    color: #999;
  }

  @media (prefers-color-scheme: dark) {
    .settings {
      color: #f0f0f0;
      background: #202020;
    }
    h2 {
      color: #aaa;
    }
    .agent-block {
      border-bottom-color: rgba(255, 255, 255, 0.08);
    }
    .hint {
      color: #999;
    }
    select,
    input[type="number"] {
      background: #2c2c2c;
      color: #f0f0f0;
      border-color: rgba(255, 255, 255, 0.15);
    }
  }
</style>
