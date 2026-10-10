# Keyboard and presentation clickers

Presentation clickers pretend to be a keyboard, so they work out of the box in the operator
window and in the output window.

| Key                     | Action           | Typical clicker button |
| ----------------------- | ---------------- | ---------------------- |
| Space, →, ↓, PageDown   | Next             | Next                   |
| ←, ↑, PageUp            | Previous         | Previous               |
| Enter, F5, Shift+F5     | Go               | "Start slideshow"      |
| Shift+→ / Shift+←       | Next / previous cue | —                   |
| B or `.`                | Blackout         | "Black screen"         |
| F                       | Freeze           | —                      |
| L                       | Logo             | —                      |
| Esc                     | Panic (logo)     | "Stop slideshow"       |

Keys are ignored while you type in a text field. Holding a key down never repeats an action.

## Changing the mapping

Edit `host-settings.json` in DECK's configuration folder (quit the app first):

- macOS: `~/Library/Application Support/io.github.kirikakaese.deck/`
- Windows: `%APPDATA%\io.github.kirikakaese.deck\`
- Linux: `~/.config/io.github.kirikakaese.deck/`

```json
{
  "keymap": {
    "x": "toggle_freeze",
    "b": "none",
    "Tab": "next"
  }
}
```

Keys use the browser's `KeyboardEvent.key` names (single characters in lower case); named keys
may be prefixed with `Shift+`, `Ctrl+`, `Alt+` or `Meta+`. Actions: `go`, `next`, `prev`,
`next_cue`, `prev_cue`, `toggle_blackout`, `toggle_freeze`, `toggle_logo`, `panic`, `none`.
A settings screen for this is planned.
