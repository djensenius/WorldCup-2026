# wc26 — World Cup 2026 TUI

A fast, keyboard-driven terminal UI for the **FIFA World Cup 2026**: schedule,
group standings, the knockout bracket, and a **live scoreboard** — built with
Rust and [ratatui](https://ratatui.rs).

> Status: under active development.

## Features

- **Matches** — fixtures by day and stage, status badges, local-timezone
  kickoff times, favourite-team filtering.
- **Live** — a compact scoreboard of in-play matches that refreshes on a fast
  cadence.
- **Standings** — the 12 group tables (A–L) with qualification highlighting.
- **Bracket** — the knockout tree (Round of 32 → Final).
- **Match detail** — goals, cards, substitutions, lineups, and team stats.
- Pluggable **data providers** (ESPN by default; API-Football and
  football-data.org optional), colour themes, an offline cache, and mouse
  support.

## Data providers

`wc26` normalizes several upstream APIs behind one interface:

| Provider           | API key | Notes                                          |
| ------------------ | ------- | ---------------------------------------------- |
| **ESPN** (default) | No      | Free, live data; the default.                  |
| API-Football       | Yes     | Richer stats; set `WC26_API_FOOTBALL_KEY`.     |
| football-data.org  | Yes     | Limited live detail; set `WC26_FOOTBALL_DATA_KEY`. |

Select a provider with `--provider <espn|api-football|football-data>` or in the
config file.

## Build & run

```sh
cargo run -p wc-tui          # or: cargo run --bin wc26
```

Requires the toolchain pinned in `rust-toolchain.toml`.

## Keybindings

| Key            | Action                |
| -------------- | --------------------- |
| `1`–`4`, `Tab` | Switch screen         |
| `j`/`k`, `↑`/`↓` | Move selection      |
| `Enter`        | Open match detail     |
| `Esc`          | Back / close          |
| `r`            | Refresh now           |
| `t`            | Cycle colour theme    |
| `?`            | Toggle help           |
| `q`            | Quit                  |

## Workspace layout

- `crates/wc-data` — normalized domain model and provider backends.
- `crates/wc-tui` — the terminal UI (binary `wc26`).

## License

Apache-2.0. See [LICENSE](LICENSE).
