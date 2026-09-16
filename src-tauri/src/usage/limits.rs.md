# `usage/limits.rs`

Enthält die fest hinterlegte Tabelle mit **geschätzten** Token-Limits pro Abo-Stufe. Wird ausschließlich im Limit-Modus `Auto` genutzt (siehe `settings.rs::LimitMode`) — weder Anthropic noch OpenAI veröffentlichen die echten Limits maschinenlesbar, daher sind das grobe, selbst gepflegte Richtwerte, keine offizielle Quelle.

## Datenstruktur

```rust
pub struct PlanTier {
    pub key: &'static str,      // z. B. "claude_max_5x" — wird in Settings gespeichert
    pub label: &'static str,    // z. B. "Claude Max 5x (approx.)" — für die UI
    pub five_hour: u64,
    pub daily: u64,
    pub weekly: u64,
}
```

Zwei statische Tabellen:

- `CLAUDE_TIERS`: `claude_pro`, `claude_max_5x`, `claude_max_20x`
- `CODEX_TIERS`: `codex_plus`, `codex_pro`

## Ablauf

1. **`plan_tiers_for(agent) -> &[PlanTier]`**
   Gibt die passende Tabelle für den Agenten zurück (`match` auf `AgentId`). Wird von zwei Stellen aufgerufen:
   - `commands.rs::list_plan_tiers` — liefert dem Frontend die Auswahlliste für das Settings-Dropdown (Key + Label, ohne die Zahlen selbst preiszugeben).
   - Intern von `limit_for`.

2. **`limit_for(agent, tier_key, window) -> Option<u64>`**
   Sucht in `plan_tiers_for(agent)` den Eintrag mit passendem `key` und liest daraus den Wert für das übergebene `window` (`FiveHour`/`Daily`/`Weekly`). `None`, falls der `tier_key` nicht existiert (z. B. noch nicht gewählt).

   Aufrufer: **`poll.rs::apply_limit()`** — dort nur, wenn `agent_settings.limit_mode == LimitMode::Auto`:

   ```rust
   LimitMode::Auto => agent_settings
       .plan_tier               // Option<String>, vom Nutzer in den Settings gewählt
       .as_deref()
       .and_then(|tier| limits::limit_for(snap.agent, tier, window)),
   ```

## Erweitern

Neue Stufe hinzufügen → einfach ein weiteres `PlanTier { .. }` in `CLAUDE_TIERS`/`CODEX_TIERS` ergänzen. Kein weiterer Code nötig, da Frontend (`SettingsView.svelte`) die Liste dynamisch per `list_plan_tiers` lädt.
