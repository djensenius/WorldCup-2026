# Keybindings

`wc26` is keyboard-first; the mouse is also supported. Keys are grouped into
global bindings (active everywhere) and per-screen bindings.

## Global

| Key                 | Action                                   |
| ------------------- | ---------------------------------------- |
| `1`–`4`             | Jump to a screen by number               |
| `Tab` / `Shift+Tab` | Next / previous screen                   |
| `r`                 | Refresh the current data now             |
| `t`                 | Cycle the colour theme (saved to config) |
| `?`                 | Toggle the help overlay                  |
| `Esc`               | Close overlay / go back                  |
| `q` / `Ctrl+C`      | Quit                                     |

The screens, in tab order, are **Matches**, **Live**, **Standings**, and
**Bracket**.

## Favourite teams

Press `*` on a selected team in **Standings** or in the **Team** view to mark it
as a favourite (press again to unmark). Favourites persist to your config and
are highlighted with a star (★) and the accent colour everywhere they appear —
Matches, Live, Standings, the Team view, and the Bracket. On Matches, `f`
filters the list down to fixtures involving a favourite.

## Matches

Opens focused on the current game (or the next kickoff when nothing is live).
Favourite teams are marked with a star (★) and highlighted in the accent colour.

| Key                | Action                              |
| ------------------ | ----------------------------------- |
| `j` / `k`, `↓`/`↑` | Move the selection                  |
| `f`                | Toggle the favourite-teams filter   |
| `Enter`            | Open match detail for the selection |

## Live

The Live screen lists matches **in play** first, then the soonest **upcoming**
fixtures, so there is always something to see even when nothing is live.

| Key                | Action                                       |
| ------------------ | -------------------------------------------- |
| `j` / `k`, `↓`/`↑` | Move across the in-play and upcoming matches |
| `Enter`            | Open match detail for the selection          |

## Standings

| Key                       | Action                                        |
| ------------------------- | --------------------------------------------- |
| `h` / `l`, `←`/`→`         | Previous / next group                         |
| `a`–`l`                   | Jump directly to a group by letter            |
| `j` / `k`, `↓`/`↑`        | Move the selected team row                    |
| `*`                       | Toggle the selected team as a favourite (★)   |
| `Enter`                   | Open the team view for the selected row       |

## Team (overlay)

Opened from **Standings** with `Enter`. Shows the team's group summary, recent
form, and full fixture list.

| Key                | Action                                      |
| ------------------ | ------------------------------------------- |
| `j` / `k`, `↓`/`↑` | Move through the team's fixtures            |
| `*`                | Toggle this team as a favourite (★)         |
| `Enter`            | Open match detail for the selected fixture  |
| `Esc`              | Close the overlay                           |

## Bracket

| Key                | Action                          |
| ------------------ | ------------------------------- |
| `h` / `l`, `←`/`→`  | Previous / next round           |
| `j` / `k`, `↓`/`↑` | Move within the round           |

## Match detail (overlay)

| Key                | Action               |
| ------------------ | -------------------- |
| `j` / `k`, `↓`/`↑` | Scroll the detail    |
| `Esc`              | Close the overlay    |

## Mouse

- **Click a tab** in the top bar to switch to that screen.
- **Scroll wheel** moves the selection on a list screen, or scrolls the match
  detail overlay when it is open.

See [data providers](data-providers.md) for configuration, and
[architecture](architecture.md) for how input is dispatched.
