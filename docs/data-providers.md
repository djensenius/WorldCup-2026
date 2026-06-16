# Data providers

WorldCup26 normalizes several upstream football APIs behind one `Provider`
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
worldcup26 --provider espn
worldcup26 --provider api-football
worldcup26 --provider football-data
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
| API-Football      | `WORLDCUP26_API_FOOTBALL_KEY`   | `provider.api_football_key`   |
| football-data.org | `WORLDCUP26_FOOTBALL_DATA_KEY`  | `provider.football_data_key`  |

```sh
export WORLDCUP26_API_FOOTBALL_KEY="your-key"
worldcup26 --provider api-football
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
via `ProjectDirs` for `dev.djensenius.worldcup26`):

- **Linux** — `~/.config/worldcup26/config.toml`
- **macOS** — `~/Library/Application Support/dev.djensenius.worldcup26/config.toml`
- **Windows** — `%APPDATA%\djensenius\worldcup26\config\config.toml`

Point at an alternate file with `worldcup26 --config <FILE>`. Common UI settings
can be overridden inline for a single run:

```sh
worldcup26 --nerd-fonts --graphics auto
worldcup26 --no-flags --theme high-contrast --timezone utc
```

A full config example:

```toml
# Teams to highlight and filter, by display name or abbreviation
# (case-insensitive).
favorites = ["Canada", "MEX"]

[provider]
# espn | api-football | football-data
kind = "espn"
# api_football_key = "..."   # prefer WORLDCUP26_API_FOOTBALL_KEY
# football_data_key = "..."  # prefer WORLDCUP26_FOOTBALL_DATA_KEY

[ui]
# Theme name; cycle at runtime with `t`. One of: world-night, world-day,
# pitch, high-contrast, catppuccin-latte, catppuccin-frappe,
# catppuccin-macchiato, catppuccin-mocha, canada.
theme = "world-night"
# Use Nerd Font glyphs for icons.
nerd_fonts = false
# Show national flags on the Live card when the terminal supports graphics.
# Matches/Standings/Team/Bracket stay text-only. Toggle at runtime with `f`.
# Graphics support is auto-detected; force it with --graphics or the
# WORLDCUP26_GRAPHICS env var (auto|kitty|iterm2|sixel|halfblocks|off).
show_flags = true
# Kickoff display: "local" (default) or "utc". A fixed whole-hour offset
# from UTC is also supported.
timezone = "local"
```

The file is created and updated automatically — for example, cycling the theme
with `t` saves the new theme name.

## Rate limits and caching

- ESPN is unofficial: WorldCup26 polls adaptively (fast only while matches are live)
  and uses conditional requests where supported to stay light.
- The keyed providers enforce hard quotas; on `429` responses WorldCup26 backs off
  and keeps showing the last good data from the offline cache.

See [architecture](architecture.md) for how polling, caching, and the provider
abstraction fit together.
