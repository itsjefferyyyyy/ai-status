# `poll.rs`

Herzstück der App: liest die lokalen Log-Dateien der Agenten ein, aktualisiert den In-Memory-Store, berechnet die Fenster-Snapshots (5h/täglich/wöchentlich), wendet die Limit-Schätzung an, informiert das Frontend und aktualisiert den Tray-Titel. Läuft alle 60s automatisch im Hintergrund — zusätzlich einmalig beim App-Start (`lib.rs::setup()`) und bei jedem manuellen `get_usage`-Aufruf (Popover-Öffnen).

## `PollState`

```rust
pub struct PollState {
    pub store: UsageStore,                          // dedupliierte Events aller Agenten
    cursors: Mutex<HashMap<AgentId, ScanCursors>>,   // Lese-Cursor pro Agent (welche Bytes wurden schon geparst)
}
```

Wird einmal in `lib.rs::setup()` erzeugt, per `app.manage()` global verfügbar gemacht und als `Arc<PollState>` sowohl an den Poll-Loop als auch an den `get_usage`-Command weitergereicht.

## `refresh(app, state, settings) -> Vec<UsageSnapshot>`

Die zentrale Funktion, in drei Phasen:

### 1. Scannen & Einlesen (nur aktivierte Agenten)

```
für jeden AgentId::ALL:
    wenn agent nicht in settings.agents aktiviert → skip (kein Datei-I/O!)
    agent.discover_files(MAX_AGE_DAYS=8)     // findet relevante .jsonl-Dateien
    für jede Datei: agent.parse_incremental(pfad, cursor)  // liest nur neu angehängte Bytes
    state.store.ingest(agent_id, neue_events)              // dedupliziert & speichert
```

Ein deaktivierter Agent wird komplett übersprungen — es wird nicht mal auf die Log-Dateien zugegriffen.

### 2. Snapshots berechnen (nur sichtbare Fenster)

```
für jeden aktivierten Agenten:
    events = state.store.events_for(agent_id)
    für jedes Fenster in UsageWindowKind::ALL:
        wenn Fenster nicht in settings.windows_visible → skip
        snap = compute_snapshot(agent, window, events, now)   // usage/blocks.rs
        apply_limit(&mut snap, agent_settings, window)         // Prozent/Limit reinrechnen
        snapshots.push(snap)
```

### 3. Frontend & Tray informieren

```rust
app.emit("usage://updated", &snapshots);   // Popover hört darauf (usage.ts)
update_tray_title(app, state, settings, now);
```

`refresh()` gibt die `snapshots` zusätzlich zurück — das nutzt `commands.rs::get_usage` für die sofortige Erstbefüllung beim Öffnen des Popovers, ohne auf den nächsten 60s-Tick warten zu müssen.

## `apply_limit(snap, agent_settings, window)`

Reine Rechenfunktion, siehe `usage/limits.rs.md` für die Datenquelle:

```
LimitMode::None   → kein Limit, snap.percent = None
LimitMode::Custom → limit = agent_settings.custom_limit
LimitMode::Auto   → limit = limits::limit_for(agent, plan_tier, window)

snap.percent = limit.map(|l| verbrauchte_tokens / l * 100)
snap.limit   = limit
```

## `update_tray_title(app, state, settings, now)`

Unabhängig von `windows_visible` — der Tray-Titel zeigt **immer** das 5-Stunden-Fenster des in den Settings gewählten `primary_agent`, auch wenn das 5h-Fenster im Popover ausgeblendet ist.

```
tray = app.tray_by_id("main-tray")
wenn primary_agent deaktiviert → Titel leeren, fertig
events = store.events_for(primary_agent)
snap = compute_snapshot(primary_agent, FiveHour, events, now)
apply_limit(...)
value = Prozent (falls Limit gesetzt) sonst format_compact_tokens(tokens_gesamt)
tray.set_title("C · 45%"  bzw.  "C · 1.2k")
```

`format_compact_tokens` rundet auf `k`/`M` (z. B. `128_000` → `"128.0k"`), damit der Tray-Text kurz bleibt.

## `spawn_poll_loop(app, state)`

```rust
tokio::time::interval(60s)
loop {
    tick.await
    settings = settings::load(&app)   // jedes Mal frisch von Disk, falls Nutzer sie geändert hat
    refresh(&app, &state, &settings)
}
```

Läuft als eigener async Task (`tauri::async_runtime::spawn`), unabhängig davon, ob gerade ein Fenster (Popover/Settings) geöffnet ist — deshalb ist der Tray-Titel immer aktuell, auch ohne dass der Nutzer je klickt.

## Aufrufer-Übersicht

| Wer ruft `refresh()` auf? | Wann? |
|---|---|
| `lib.rs::setup()` | einmalig beim App-Start (Tray nicht erst nach 60s befüllen) |
| `poll::spawn_poll_loop` | alle 60s |
| `commands.rs::get_usage` | jedes Mal, wenn das Popover geöffnet wird |
