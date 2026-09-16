# AI Status

AI Status is a macOS menu bar app that shows your 5-hour, daily, and weekly usage of AI coding agents, updated every minute. A settings window lets you choose which usage windows are shown and which agents are tracked.

## Supported agents

- **Claude Code** — reads local session logs (`~/.claude/projects/**/*.jsonl`)
- **Codex CLI** — reads local session logs (`~/.codex/sessions/YYYY/MM/DD/*.jsonl`)

Both read purely local files; no login or API key is required. **ChatGPT (web/subscription)** is not supported yet — OpenAI doesn't offer an official API for the consumer subscription's usage/limit display, only an undocumented endpoint that would require a browser session cookie, which was judged too fragile and ToS-risky to build on for now.

Neither data source exposes your actual plan limit, only tokens actually consumed. Per agent, you can choose how usage is displayed in Settings:

- **No estimate** — raw token counts only
- **Auto-estimate** — a percentage against a built-in, clearly-approximate table of known plan tiers
- **Custom limit** — a percentage against a limit you enter yourself

An optional **API fee calculator** (off by default, toggle it in Settings) estimates what your local usage would have cost as pay-as-you-go API calls. It only prices models it has actually seen in your local logs — using each log's own reported cost when available, and otherwise a reference price table for known model families — and shows the estimate next to token counts in the popover and tray title.

## Development

Requires [pnpm](https://pnpm.io) and a Rust toolchain (`rustc`/`cargo`).

```sh
pnpm install
pnpm tauri dev    # run the app
pnpm tauri build  # produce a signed .app/.dmg
```

See `CLAUDE.md` for the full architecture map and additional commands (tests, lint).
