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

## Matches

| Key                | Action                              |
| ------------------ | ----------------------------------- |
| `j` / `k`, `↓`/`↑` | Move the selection                  |
| `f`                | Toggle the favourite-teams filter   |
| `Enter`            | Open match detail for the selection |

## Live

| Key                | Action                              |
| ------------------ | ----------------------------------- |
| `j` / `k`, `↓`/`↑` | Move the selection                  |
| `Enter`            | Open match detail for the selection |

## Standings

| Key                       | Action                          |
| ------------------------- | ------------------------------- |
| `h` / `l`, `←`/`→`         | Previous / next group           |
| `a`–`l`                   | Jump directly to a group by letter |

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
