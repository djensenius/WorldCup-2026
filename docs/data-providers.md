# Data providers

`wc26` normalizes several upstream football APIs behind one `Provider`
interface, so every screen works the same regardless of the selected source.
ESPN is the default and needs no API key.

| Provider            | Key needed | Strengths                                  | Limitations                                  |
| ------------------- | ---------- | ------------------------------------------ | -------------------------------------------- |
| **ESPN** (default)  | No         | Free, live now, full timeline and lineups  | Unofficial; be conservative with polling     |
| API-Football        | Yes        | Rich stats and lineups, fast live cadence  | Free tier ~100 requests/day                  |
| football-data.org   | Yes        | Simple and stable                          | Limited live event granularity on free tier  |

## Selecting a provider

For a single run, pass the flag:

```sh
wc26 --provider espn
wc26 --provider api-football
wc26 --provider football-data
```

To make it the default, set it in the config file (see
[configuration](#configuration)):

```toml
[provider]
kind = "api-football"
```

An unknown provider name falls back to the default (ESPN) rather than failing.

## API keys

The keyed providers read their credentials from environment variables, which
take precedence over any value stored in the config file:

| Provider          | Environment variable      | Config key                    |
| ----------------- | ------------------------- | ----------------------------- |
| API-Football      | `WC26_API_FOOTBALL_KEY`   | `provider.api_football_key`   |
| football-data.org | `WC26_FOOTBALL_DATA_KEY`  | `provider.football_data_key`  |

```sh
export WC26_API_FOOTBALL_KEY="your-key"
wc26 --provider api-football
```

Where to obtain a key:

- **API-Football** — sign up at <https://www.api-football.com/>. The key is sent
  as the `x-apisports-key` request header.
- **football-data.org** — register at <https://www.football-data.org/>. The
  token is sent as the `X-Auth-Token` request header.

Prefer environment variables or a config file with restrictive permissions over
committing keys anywhere.

## Configuration

Configuration is TOML stored in the platform configuration directory (resolved
via `ProjectDirs` for `dev.djensenius.wc26`):

- **Linux** — `~/.config/wc26/config.toml`
- **macOS** — `~/Library/Application Support/dev.djensenius.wc26/config.toml`
- **Windows** — `%APPDATA%\djensenius\wc26\config\config.toml`

Point at an alternate file with `wc26 --config <FILE>`. A full example:

```toml
# Teams to highlight and filter, by display name or abbreviation
# (case-insensitive).
favorites = ["Canada", "MEX"]

[provider]
# espn | api-football | football-data
kind = "espn"
# api_football_key = "..."   # prefer WC26_API_FOOTBALL_KEY
# football_data_key = "..."  # prefer WC26_FOOTBALL_DATA_KEY

[ui]
# Theme name; cycle at runtime with `t`. One of: world-night, world-day,
# pitch, high-contrast, catppuccin-latte, catppuccin-frappe,
# catppuccin-macchiato, catppuccin-mocha, canada.
theme = "world-night"
# Use Nerd Font glyphs for icons.
nerd_fonts = false
# Show colored ASCII-art flags (toggle at runtime with `f` on the Live screen).
show_flags = true
# Kickoff display: "local" (default) or "utc". A fixed whole-hour offset
# from UTC is also supported.
timezone = "local"
```

The file is created and updated automatically — for example, cycling the theme
with `t` saves the new theme name.

## Rate limits and caching

- ESPN is unofficial: `wc26` polls adaptively (fast only while matches are live)
  and uses conditional requests where supported to stay light.
- The keyed providers enforce hard quotas; on `429` responses `wc26` backs off
  and keeps showing the last good data from the offline cache.

See [architecture](architecture.md) for how polling, caching, and the provider
abstraction fit together.
