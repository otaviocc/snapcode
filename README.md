# snapcode

Render a code snippet to an image — a syntax-highlighted window on a background,
as a PNG or SVG. One cross-platform binary that is both a scriptable CLI and an
interactive TUI with a live preview in the terminal.

```sh
snapcode Feed.swift --line-numbers -o feed.png
snapcode tui Feed.swift
```

## Why

This replaces a Python/Pillow script that only understood Swift, had one
hardcoded theme, and rendered differently on every machine because it picked
whatever monospace font the host happened to have. snapcode handles 220
languages, ships its font inside the binary so output is reproducible, and lets
you see what you are making before you make it.

## Install

```sh
cargo install --path crates/snapcode-cli
```

## Use

```text
snapcode [INPUT] [-o OUTPUT] [FLAGS]
```

`INPUT` is a file or `-` for stdin; a piped stdin is detected automatically.
snapcode never reads your clipboard on its own — to render what is on it, pipe
it in explicitly. `OUTPUT` defaults to the input name with a new extension; `-`
writes to stdout, leaving progress on stderr so the pipe stays clean.

```sh
snapcode main.rs                             # -> main.png
snapcode main.rs -o out.svg                  # SVG, from the extension
pbpaste | snapcode - --lang swift --copy     # clipboard in, clipboard out
snapcode main.rs -o - | pbcopy               # straight down a pipe
snapcode main.rs --watch                     # re-render on every save
```

### Appearance

| Flag | What it does |
| --- | --- |
| `--theme`, `--syntax-theme` | Window chrome and token colors, independently |
| `--bg` | A color, `linear:<angle>:<c1>,<c2>`, `radial:<c1>,<c2>`, or an image path |
| `--bg-fit`, `--bg-blur`, `--bg-darken` | How an image background is placed and treated |
| `--padding`, `--margin`, `--radius`, `--border` | Window geometry |
| `--title`, `--no-titlebar`, `--traffic-lights` | Titlebar; the title defaults to the filename |
| `--shadow-blur`, `--shadow-color`, `--no-shadow` | Drop shadow |
| `--font`, `--font-size`, `--line-height`, `--ligatures` | Type |
| `--scale`, `--dpi` | 1 normal, 2 retina, 3+ print; DPI is written into the PNG |

### Content

| Flag | What it does |
| --- | --- |
| `--lang` | Override language detection |
| `--line-numbers`, `--line-start`, `--gutter-separator` | Gutter |
| `--highlight-lines 3-5,9` | Emphasize those lines, dim the rest |
| `--diff` | Render leading `+`/`-` as diff line backgrounds |
| `--tab-width`, `--max-width`, `--wrap` | Text handling |

`snapcode themes` and `snapcode languages` list what is available.

## TUI

`snapcode tui FILE` opens a settings form beside a live preview of the real
image — the same renderer the CLI uses, so what you see is what you export.
Without a FILE it starts empty; press `i` to open `$EDITOR`, paste a snippet,
and save. `i` works with a file too — it edits a temp copy, so your file is
never written to. A GUI editor must be told to wait (`EDITOR='code --wait'`,
`EDITOR='subl -w'`); one that returns immediately hands back an unchanged
snippet, and snapcode says so rather than claiming an update.

```text
j / k, ↓ / ↑     move between settings      e   export a PNG
h / l, ← / →     adjust the selection       c   copy to the clipboard
space            toggle a boolean           s   save as your defaults
enter, /         search a long list         p   quit, printing the equivalent command
g / G            first / last setting       r   reload the source file
                                            i   edit the snippet in $EDITOR
```

Settings are grouped into THEME, CODE, TYPE and WINDOW. Most are stepped with
left/right, but the ones backed by long lists — theme, syntax theme, and
language — open a search box instead: type to narrow, arrow to choose, enter to
accept. Language shows what detection chose (`Swift (auto)`) until you override
it, and `(automatic)` at the top of its list hands it back.

`p` is the useful one: tune it by eye, then get the command line back out.

The form takes its colors from the theme you have selected, so it previews the
theme as well as describing it. Every combination of chrome and syntax theme is
held to a WCAG contrast ratio, so a mismatched pair stays readable.

The preview uses the terminal's own graphics protocol — kitty, Ghostty, WezTerm,
iTerm2 — and falls back to unicode half-blocks anywhere else, including inside
tmux, where graphics escapes are not forwarded. Override the choice with
`SNAPCODE_IMAGE_PROTOCOL=kitty|iterm2|sixel|halfblocks`.

## Configuration

Precedence, lowest to highest: built-in defaults, `~/.config/snapcode/config.toml`,
`./snapcode.toml`, then flags. Each layer overrides only the fields it mentions,
so a repo can pin a theme without restating everything else.

```sh
snapcode config path        # where the config file lives
snapcode config print       # the effective configuration, with flags applied
snapcode config save        # write it there
```

Drop `.toml` chrome themes and `.tmTheme` syntax themes in
`~/.config/snapcode/themes/` and they show up in `snapcode themes`.

## How it works

```text
source ──> syntect highlight ──> cosmic-text shaping ──> Scene ──┬─> tiny-skia ──> PNG
                                                                 └─> SVG
```

Both backends consume one `Scene` of drawing primitives, so PNG and SVG cannot
drift apart — a test renders each and compares them pixel by pixel.

Three decisions worth knowing about:

- **The font is embedded.** JetBrains Mono ships inside the binary, so the same
  command produces the same pixels everywhere. `--font` still resolves system
  families, and system fonts are always loaded for fallback, so CJK and emoji
  render instead of turning into tofu.
- **Widths come from glyph advances, never ink bounding boxes.** An ink box
  silently drops leading and trailing whitespace, which would collapse the
  indentation that makes code readable.
- **SVG glyphs are path outlines, not `<text>`.** No embedded font, no viewer
  substitution, and it matches the PNG exactly. Image backgrounds are the one
  thing SVG output drops, and it says so when it does.
- **The terminal's image protocol is detected from the environment and a
  `TIOCGWINSZ` ioctl, never by querying over stdin.** `ratatui-image`'s stdio
  query leaves a reader thread blocked on stdin when a terminal does not answer
  within its timeout, and that thread then competes with the event loop for
  keystrokes. `tui/preview.rs` exists to avoid it.

## Development

```sh
make check                 # fmt, clippy, markdown, and the full test suite
make test-render           # just the pixel-level and SVG-parity tests
```

The rendering tests assert on actual pixels — geometry, theme colors at known
points, determinism — because a layout bug still produces a perfectly valid PNG.

## License

MIT — see [LICENSE](LICENSE). The bundled JetBrains Mono is licensed separately
under the SIL Open Font License 1.1; its text is in
`crates/snapcode-core/assets/fonts/OFL.txt`.
