# snapcode

[![CI](https://img.shields.io/github/actions/workflow/status/otaviocc/snapcode/ci.yml?branch=main)](https://github.com/otaviocc/snapcode/actions/workflows/ci.yml)
[![GitHub release](https://img.shields.io/github/v/release/otaviocc/snapcode)](https://github.com/otaviocc/snapcode/releases/latest)
[![crates.io](https://img.shields.io/crates/v/snapcode.svg)](https://crates.io/crates/snapcode)
[![license](https://img.shields.io/github/license/otaviocc/snapcode.svg)](https://github.com/otaviocc/snapcode/blob/main/LICENSE)

Turn a code snippet into a shareable image — a syntax-highlighted window on a
background, as a PNG or an SVG. Point `snapcode` at a file and it picks the
language, sizes the window to the code and writes the image next to it.

It is one Rust binary that is both a scriptable CLI and a TUI with a live
preview in the terminal. 220 languages, seventeen themes and the font are all
compiled in, so the same command produces the same pixels on every machine.

<img width="1613" height="1041" alt="screenshot" src="https://github.com/user-attachments/assets/73649684-adaa-40b0-9eca-798911a0ee63" />

## Install

### Cargo

```sh
cargo install snapcode --locked
```

### Prebuilt binaries

Grab an archive from the [latest release](https://github.com/otaviocc/snapcode/releases/latest)
and put the binary on your `PATH`.

| Platform | Archive |
| --- | --- |
| Linux, x86-64 | `snapcode-<version>-x86_64-unknown-linux-gnu.tar.gz` |
| Linux, ARM64 | `snapcode-<version>-aarch64-unknown-linux-gnu.tar.gz` |
| macOS, universal | `snapcode-<version>-macos-universal.tar.gz` |
| Windows, x86-64 | `snapcode-<version>-x86_64-pc-windows-msvc.zip` |

## Use

```sh
snapcode main.rs                             # -> main.png
snapcode main.rs -o out.svg                  # SVG, from the extension
snapcode main.rs --theme nord --line-numbers
pbpaste | snapcode - --lang swift --copy     # clipboard in, clipboard out
snapcode main.rs -o - | pbcopy               # straight down a pipe
snapcode main.rs --watch                     # re-render on every save
```

`INPUT` is a file or `-` for stdin; a piped stdin is detected automatically.
snapcode never reads your clipboard on its own — to render what is on it, pipe
it in explicitly. `OUTPUT` defaults to the input name with a new extension; `-`
writes to stdout, leaving progress on stderr so the pipe stays clean.

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

Seventeen themes ship built in — `warm`, `midnight`, `paper`, `dracula`, `nord`,
`gruvbox`, `solarized-dark`, `solarized-light`, `catppuccin-mocha`,
`catppuccin-latte`, `one-dark`, `github-light`, `tokyo-night`, `rose-pine`,
`everforest`, `kanagawa` and `default-plus`. Each brings its own token colors, so
`--theme nord` is all you need; `--syntax-theme` picks those separately from 38,
if you want a light window carrying a dark theme's palette.

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

```text
j / k, ↓ / ↑     move between settings      e   export a PNG
h / l, ← / →     adjust the selection       c   copy to the clipboard
space            toggle a boolean           s   save as your defaults
enter, /         search a long list         p   quit, printing the equivalent command
g / G            first / last setting       r   reload the source file
                                            i   edit the snippet in $EDITOR
```

`p` is the useful one: tune it by eye, then get the command line back out.

Without a FILE it starts empty — press `i` to open `$EDITOR`, paste a snippet
and save. `i` works with a file too, editing a temp copy so your own file is
never written to. A GUI editor has to be told to wait (`EDITOR='code --wait'`,
`EDITOR='subl -w'`).

The preview uses the terminal's own graphics protocol — kitty, Ghostty, WezTerm,
iTerm2 — and falls back to unicode half-blocks anywhere else, including inside
tmux. Override the choice with
`SNAPCODE_IMAGE_PROTOCOL=kitty|iterm2|sixel|halfblocks`.

## Configuration

Precedence, lowest to highest: built-in defaults,
`~/.config/snapcode/config.toml`, `./snapcode.toml`, then flags. Each layer
overrides only the fields it mentions, so a repo can pin a theme without
restating everything else.

```sh
snapcode config path        # where the config file lives
snapcode config print       # the effective configuration, with flags applied
snapcode config save        # write it there
```

Drop `.toml` chrome themes and `.tmTheme` syntax themes in
`~/.config/snapcode/themes/` and they show up in `snapcode themes`.

## License

MIT — see [LICENSE](LICENSE). The bundled JetBrains Mono is licensed separately
under the SIL Open Font License 1.1; its text is in
`crates/snapcode-core/assets/fonts/OFL.txt`.
