# Architecture

`wc26` is a two-crate Cargo workspace. The data layer is provider-agnostic and
has no terminal dependencies; the UI layer renders that normalized data and
owns all polling, navigation, and rendering.

```text
+-------------------------------------------------------------+
|  wc-tui (binary: wc26)                                      |
|                                                             |
|  App  ──  EventLoop (crossterm EventStream + tick)          |
|   │         │                                               |
|   │         ├── Key / Mouse / Resize  ── navigation         |
|   │         └── Tick ── adaptive polling + cache persist    |
|   │                                                         |
|   ├── Poller<T> (per data domain) ── mpsc ◄── async fetch   |
|   ├── Cache (offline JSON, best-effort)                     |
|   └── ui:: render ── screens + widgets                      |
+----------------------────────────-------────────────────────+
                          │ calls
                          ▼
+-------------------------------------------------------------+
|  wc-data (library)                                          |
|                                                             |
|  Provider (enum dispatch)                                   |
|   ├── EspnProvider          (default, no key)               |
|   ├── ApiFootballProvider   (x-apisports-key)               |
|   └── FootballDataProvider  (X-Auth-Token)                  |
|                                                             |
|  each backend: DTOs → mapper → domain model                 |
|  Http (shared reqwest client, rustls, timeout + retry)      |
+-------------------------------------------------------------+
```

## Crates

- **`wc-data`** — the normalized domain model (`Match`, `Group`,
  `GroupStanding`, `Calendar`, `Bracket`, `MatchDetail`) and the pluggable
  provider backends. It depends on `reqwest`, `serde`, and `time`, but never on
  `ratatui` or `crossterm`. This keeps the data layer independently testable
  and reusable.
- **`wc-tui`** — the terminal application (binary `wc26`). It owns the `App`
  state machine, the async event loop, configuration, the offline cache, and
  all rendering.

## Data flow

1. On startup the `App` seeds each `Poller` from the offline cache so the UI has
   something to show immediately, then schedules a fresh fetch.
2. The event loop selects over the crossterm `EventStream` and a periodic tick.
3. Each tick drains completed fetches into the relevant `Poller`, persists fresh
   payloads to the cache, and expires transient toasts.
4. Rendering reads only from `App` state; it never performs I/O.

### The `Remote<T>` / `Poller<T>` pattern

Every data domain (scoreboard, standings, calendar, bracket, match detail) is
loaded through a `Poller<T>` that wraps a `Remote<T>` load state — one of
`Loading`, `Ready { value, fetched_at }`, or `Failed { error, at }`. Fetches run
on the Tokio runtime and report results back over an `mpsc` channel.

When a refresh fails, the last good value stays visible and the status bar notes
how long ago the failure happened; the poller only enters the `Failed` state if
there was never a successful value.

### Adaptive polling

Polling cadence adapts to the tournament state: a fast cadence while any match
is in play and a slow cadence (minutes) otherwise. The currently open match
detail polls on its own faster cadence while the overlay is visible. This keeps
the unofficial ESPN endpoint usage conservative while staying live during games.

## Provider abstraction

`Provider` is an enum that dispatches to one backend at runtime, avoiding an
`async-trait` dependency by using native `async fn` in inherent methods. Each
backend deserializes its own upstream DTOs and maps them into the shared domain
model, so the UI is identical regardless of the selected source. See
[data providers](data-providers.md) for the per-provider details and keys.

## Offline cache

The last good normalized payload for each domain is written as JSON under the
platform cache directory (`ProjectDirs` for `dev.djensenius.wc26`). Reads and
writes are best-effort: a missing or unreadable cache never blocks startup. The
status bar shows a cached indicator until the first successful live fetch of the
session completes.
