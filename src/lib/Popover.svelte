<script lang="ts">
  import { onMount } from "svelte";
  import { getUsage, onUsageUpdated } from "./usage";
  import { openSettingsWindow } from "./settings";
  import { resizeMainWindow } from "./window";
  import { AGENT_LABELS, WINDOW_LABELS, tokenTotal, type AgentId, type UsageSnapshot } from "./types";

  let snapshots = $state<UsageSnapshot[]>([]);
  let now = $state(Date.now());
  let rootEl: HTMLElement;

  onMount(() => {
    let unlisten: (() => void) | undefined;

    getUsage().then((s) => (snapshots = s));
    onUsageUpdated((s) => (snapshots = s)).then((fn) => (unlisten = fn));

    // Only drives the "resets in Xh Ym" / "updated Xs ago" text between the
    // minute-granularity backend pushes — no data fetching happens here.
    const tick = setInterval(() => (now = Date.now()), 1000);

    // The window has no content-based sizing of its own (it's a borderless
    // popover), so we measure our own rendered height and ask the backend to
    // resize+re-anchor the window to match, instead of carrying fixed empty
    // space for whatever the longest possible content is.
    const observer = new ResizeObserver(() => {
      if (rootEl) resizeMainWindow(rootEl.offsetHeight);
    });
    observer.observe(rootEl);

    return () => {
      unlisten?.();
      clearInterval(tick);
      observer.disconnect();
    };
  });

  const byAgent = $derived.by(() => {
    const map = new Map<AgentId, UsageSnapshot[]>();
    for (const s of snapshots) {
      const list = map.get(s.agent) ?? [];
      list.push(s);
      map.set(s.agent, list);
    }
    return map;
  });

  function formatCountdown(endMs: number): string {
    const remainingMs = endMs - now;
    if (remainingMs <= 0) return "resets shortly";
    const totalMinutes = Math.floor(remainingMs / 60000);
    const hours = Math.floor(totalMinutes / 60);
    const minutes = totalMinutes % 60;
    return `resets in ${hours}h ${minutes}m`;
  }

  function formatUpdatedAgo(): string {
    if (snapshots.length === 0) return "";
    const latest = Math.max(...snapshots.map((s) => s.updated_at_ms));
    const secs = Math.max(0, Math.floor((now - latest) / 1000));
    return `updated ${secs}s ago`;
  }

  function formatCost(snap: UsageSnapshot): string {
    if (snap.estimated_cost_usd === null) return "";
    const amount = `$${snap.estimated_cost_usd.toFixed(2)}`;
    return snap.cost_incomplete ? `${amount}+` : amount;
  }
</script>

<main class="popover" bind:this={rootEl}>
  <header>
    <h1>AI Status</h1>
    <button class="icon-button" onclick={() => openSettingsWindow()} title="Settings" aria-label="Settings">
      ⚙
    </button>
  </header>

  {#if snapshots.length === 0}
    <p class="empty">No usage data yet. Enable an agent in Settings.</p>
  {:else}
    {#each [...byAgent.entries()] as [agent, agentSnapshots] (agent)}
      <section class="agent">
        <h2>{AGENT_LABELS[agent]}</h2>
        {#each agentSnapshots as snap (snap.window)}
          <div class="window-row">
            <div class="window-row-top">
              <span class="window-label">{WINDOW_LABELS[snap.window]}</span>
              <span class="tokens">
                {tokenTotal(snap.totals).toLocaleString()} tokens
                {#if snap.estimated_cost_usd !== null}
                  <span class="cost">· {formatCost(snap)}</span>
                {/if}
              </span>
            </div>
            {#if snap.percent !== null}
              <div class="bar">
                <div class="bar-fill" style="width: {Math.min(100, snap.percent)}%"></div>
              </div>
              <div class="window-row-bottom">
                <span class="percent">{snap.percent.toFixed(0)}% of estimate</span>
                {#if snap.window === "five_hour" && snap.window_end_ms}
                  <span class="countdown">{formatCountdown(snap.window_end_ms)}</span>
                {/if}
              </div>
            {:else if snap.window === "five_hour" && snap.window_end_ms}
              <div class="window-row-bottom">
                <span class="countdown">{formatCountdown(snap.window_end_ms)}</span>
              </div>
            {/if}
          </div>
        {/each}
      </section>
    {/each}
  {/if}

  <footer>{formatUpdatedAgo()}</footer>
</main>

<style>
  .popover {
    display: flex;
    flex-direction: column;
    /* No fixed/viewport height: the window is resized to match this
       element's natural content height (see the ResizeObserver above).
       max-height is just a safety cap matching the backend's clamp. */
    max-height: 640px;
    padding: 12px 14px;
    box-sizing: border-box;
    font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
    font-size: 13px;
    color: #1a1a1a;
    background: rgba(255, 255, 255, 0.9);
    overflow-y: auto;
  }

  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 8px;
  }

  h1 {
    font-size: 14px;
    font-weight: 600;
    margin: 0;
  }

  .icon-button {
    border: none;
    background: transparent;
    cursor: pointer;
    font-size: 14px;
    padding: 2px 6px;
    border-radius: 6px;
  }
  .icon-button:hover {
    background: rgba(0, 0, 0, 0.06);
  }

  .empty {
    color: #666;
    font-size: 12px;
  }

  .agent {
    margin-bottom: 10px;
  }

  .agent h2 {
    font-size: 12px;
    font-weight: 600;
    color: #555;
    margin: 0 0 4px 0;
    text-transform: uppercase;
    letter-spacing: 0.03em;
  }

  .window-row {
    padding: 4px 0;
  }

  .window-row-top {
    display: flex;
    justify-content: space-between;
  }

  .window-label {
    color: #333;
  }

  .tokens {
    font-variant-numeric: tabular-nums;
    color: #111;
  }

  .cost {
    color: #777;
  }

  .bar {
    height: 4px;
    background: rgba(0, 0, 0, 0.08);
    border-radius: 2px;
    margin-top: 4px;
    overflow: hidden;
  }

  .bar-fill {
    height: 100%;
    background: #396cd8;
    border-radius: 2px;
  }

  .window-row-bottom {
    display: flex;
    justify-content: space-between;
    margin-top: 2px;
    font-size: 11px;
    color: #777;
  }

  footer {
    margin-top: auto;
    padding-top: 6px;
    font-size: 10px;
    color: #999;
    text-align: right;
  }

  @media (prefers-color-scheme: dark) {
    .popover {
      color: #f0f0f0;
      background: rgba(30, 30, 30, 0.9);
    }
    .icon-button:hover {
      background: rgba(255, 255, 255, 0.1);
    }
    .agent h2 {
      color: #aaa;
    }
    .window-label {
      color: #ddd;
    }
    .tokens {
      color: #fff;
    }
    .cost {
      color: #999;
    }
    .bar {
      background: rgba(255, 255, 255, 0.12);
    }
    .window-row-bottom {
      color: #999;
    }
    footer {
      color: #777;
    }
  }
</style>
