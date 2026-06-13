# wc26 — World Cup 2026 TUI

A fast, keyboard-driven terminal UI for the **FIFA World Cup 2026**: schedule,
group standings, the knockout bracket, and a **live scoreboard** — built with
Rust and [ratatui](https://ratatui.rs).

> Status: under active development.

## Features

- **Matches** — fixtures by day and stage, status badges, local-timezone
  kickoff times, and favourite-team filtering; opens on the current (or next)
  game.
- **Live** — a glanceable "Live Activity" card: a large block-digit score
  flanked by real national flags (inline images on Kitty / Ghostty / WezTerm /
  iTerm2 / Sixel terminals; omitted on terminals without graphics support), the
  clock, and the most recent event; previews the next kickoff with a countdown
  when nothing is in play.
- **Standings** — the 12 group tables (A–L) with qualification highlighting,
  team-row navigation, and `Enter` to open a team view.
- **Team** — opened from Standings: a team's group summary, recent form, and
  full fixture list.
- **Bracket** — the knockout tree (Round of 32 → Final).
- **Match detail** — goals, cards, substitutions, lineups, and team stats.
- **Favourite teams** — star teams with `*` from Standings or the team view;
  favourites are highlighted (★) across every screen and can filter the Matches
  list.
- Small inline **flags** beside each team in the Matches, Standings, and Team
  lists (colored half-blocks, on any terminal).
- Pluggable **data providers** (ESPN by default; API-Football and
  football-data.org optional), nine colour themes (including Catppuccin and a
  Government of Canada palette), real national flags via terminal graphics
  protocols, an offline cache, and mouse support.

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

| Key                 | Action                                  |
| ------------------- | --------------------------------------- |
| `1`–`4`             | Jump to a screen by number              |
| `Tab` / `Shift+Tab` | Next / previous screen                  |
| `j`/`k`, `↓`/`↑`    | Move selection                          |
| `Enter`             | Open match detail (team view on Standings) |
| `f`                 | Toggle favourites filter (Matches)      |
| `*`                 | Toggle favourite team (Standings, Team) |
| `h`/`l`, `←`/`→`    | Switch group / round                    |
| `r`                 | Refresh now                             |
| `t`                 | Cycle colour theme                      |
| `?`                 | Toggle help                             |
| `Esc`               | Back / close                            |
| `q`                 | Quit                                    |

The full list, including per-screen and mouse bindings, is in
[docs/keybindings.md](docs/keybindings.md).

## Workspace layout

- `crates/wc-data` — normalized domain model and provider backends.
- `crates/wc-tui` — the terminal UI (binary `wc26`).

## Documentation

- [Architecture](docs/architecture.md) — crates, data flow, polling, cache.
- [Data providers](docs/data-providers.md) — providers, API keys, configuration.
- [Keybindings](docs/keybindings.md) — full keyboard and mouse reference.

## License

Apache-2.0. See [LICENSE](LICENSE).

Bundled national-flag artwork is from
[flag-icons](https://github.com/lipis/flag-icons) (MIT); see
[crates/wc-tui/assets/flags/ATTRIBUTION.md](crates/wc-tui/assets/flags/ATTRIBUTION.md).
