# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

`snapcode` renders a code snippet to an image: a syntax-highlighted window on a
background, as PNG or SVG. One cross-platform binary that is both a scriptable
CLI and a TUI with a live in-terminal preview. It replaces a Python/Pillow
script that used to live in `otaviocc/dotfiles`'s
`claude/.claude/skills/code-snippet-image/`; that skill now shells out to this
binary. See `README.md` for the user-facing flag reference.

## Commands

```sh
make check                              # fmt-check + lint + lint-md + test: the pre-commit gate
make build                              # cargo build --workspace
make test                               # cargo test --workspace
make test-render                        # only tests/render.rs and tests/svg_parity.rs
make lint                               # cargo clippy --workspace --all-targets -- -D warnings
make run ARGS="snippet.swift --line-numbers"
make tui FILE=snippet.swift
make install                            # cargo install --path crates/snapcode-cli --locked --force
```

Run one test: `cargo test -p snapcode-core --test render traffic_lights`, or
`cargo test -p snapcode fields::` for a unit-test module.

## Architecture

Two crates. `snapcode-core` is the renderer and has no CLI or TUI dependencies;
`crates/snapcode-cli/` holds the binary, whose package is named `snapcode` so
that `cargo install snapcode` works — the directory keeps the longer name, so
`-p snapcode` and `crates/snapcode-cli` both refer to it. The pipeline is:

```text
source -> syntect highlight -> cosmic-text shape -> compose a Scene -> raster | svg
```

- `scene.rs` is the load-bearing idea. Composition produces a flat list of
  drawing primitives, and `backend/raster.rs` (tiny-skia) and `backend/svg.rs`
  are two renderers *of that one scene*, not two parallel drawing paths.
  `tests/svg_parity.rs` rasterizes the SVG with `resvg` and diffs it against the
  PNG, so a change to one backend that forgets the other fails there.
- `compose.rs` owns all geometry and sizes the window to its content, so callers
  never pass image dimensions. Its base measurements (30 padding, 50 titlebar,
  10 radius, traffic lights of radius 6 inset by 20) are inherited from the
  Python script so a default render stays recognizable; all are configurable and
  every value is multiplied by `config.scale` on the way to pixels.
- `layout.rs` measures with shaped glyph *advances*, never ink bounding boxes.
  An ink box drops leading and trailing whitespace, which collapses the
  indentation that makes code readable. Do not "simplify" this to a bbox call.
- `font.rs` embeds JetBrains Mono so a default render is byte-identical across
  machines. System fonts are still loaded for fallback (CJK, emoji).
  `FontStack::split` exists because `set_rich_text` needs `&mut FontSystem`
  while the `Attrs` it consumes borrow the family name.
- `backend/raster.rs` blurs shadows over the shadow's bounding box plus
  three box radii, not the whole canvas, and its vertical pass walks *rows*
  with a running per-column sum. Walking columns strides `width*4` bytes per
  step and misses the cache on nearly every read; it was most of a render.
  A unit test in that file pins the fast blur to a naive reference: keep it byte-exact.
- The TUI never renders on the event-loop thread. `tui/worker.rs` takes jobs
  tagged with a generation, keeps only the newest of any that queued up, and
  the UI drops a result whose generation is stale. Do not call
  `render_raster` from `draw` or `handle_key`; a key held down would queue a
  render per repeat. `[profile.dev.package."*"]` optimizes dependencies so
  `make tui` is usable from a debug build. The preview job alone gets
  `shadow = None` (`preview_config`); `App.config` keeps the shadow so export,
  copy, save and `p` still see it. Do not strip it from `App.config`.
- `theme.rs` has two independent axes: a chrome theme (`.toml`, window frame)
  and a syntax theme (`.tmTheme`). Every chrome theme names the syntax theme it
  was built around, and `config.syntax_theme` defaults to the *empty string* so
  that pairing is what renders; a non-empty value overrides it. Do not give it a
  concrete default — that silently pins one palette onto all seventeen frames.
  A built-in theme's file stem, its `name` field and its key in `BUILTIN_CHROME`
  must all match, and `load_dir` keys user themes by stem for the same reason.
  `resolve_color` resolves the terminal-only
  encodings that `ansi`/`base16` use — they store an ANSI palette index in the
  red channel, which renders as invisible text if taken at face value.
- `highlight.rs` wraps `two-face`'s syntax set (~220 languages), not syntect's
  bundled 75, which has no Swift.
- `tui/palette.rs` derives the TUI's colors from the selected theme. Chrome and
  syntax themes are picked independently, so a light window can carry a dark
  theme's pastels; every color is held to a WCAG contrast ratio against the
  background it will sit on, and a test walks all ~100 pairings.
- `editor.rs` hands the snippet to `$EDITOR` and takes the text back. It always
  edits a *temp copy* seeded from the in-memory buffer, never the user's input
  file — the TUI is a renderer, not an editor of your sources. The TUI reads no
  clipboard: `io.rs` resolves input to a file, stdin, or nothing, and nothing
  reaches into the clipboard on the way in. `--copy` and the TUI's `c` still
  write to it, because those are asked for.
- Suspending the TUI (`App::edit_snippet`) must leave and re-enter the alternate
  screen in a balanced pair on *every* path, including a failed editor, or the
  user is stranded in raw mode. Afterwards `self.preview` is dropped, not
  reused: a kitty/iterm2 `StatefulProtocol` holds placements that the screen
  churn invalidates, so it has to be rebuilt. Do not "drain leftover keystrokes"
  there with a `while event::poll(ZERO) { event::read() }` loop: `poll` reports
  ready on a *partial* escape sequence and `read` then blocks forever waiting
  for the rest, which hangs the TUI at 0% CPU with no way out. An edit that comes back byte-identical
  is reported as unchanged, not as an update: a GUI `$EDITOR` without a wait flag
  exits instantly, and claiming success there would be a lie the user only
  discovers after the temp file is gone.
- `tui/preview.rs` resolves the terminal's image protocol from environment
  variables and a `TIOCGWINSZ` ioctl, never by querying over stdin.
  `ratatui-image`'s stdio query leaves a reader thread blocked on stdin when a
  terminal does not answer within its timeout; that thread then competes with
  the event loop and swallows roughly every other keystroke. Do not replace this
  with `Picker::from_query_stdio`.

## Code conventions

- **No comments in Rust.** A file may carry a single `//!` line saying what it
  is, for navigation. Nothing else: no `///`, no `//`. Put the explanation in
  the commit message, or in this file's Architecture section.
- The one exception is `clap` in `cli.rs`: use `#[arg(help = "...")]` and
  `#[command(about = "...")]`, never a `///` doc comment. clap reads doc
  comments as `--help` text, so a `///` there is functional rather than
  narrative — and stripping them silently empties `--help`.
  `every_argument_and_subcommand_documents_itself` guards this.
- Every `.rs` file starts with `// SPDX-License-Identifier: MIT` as its first
  line, above the `//!` line.
- CLI flags are `Option<T>` so an unset flag defers to the config file rather
  than a clap default silently overwriting it. Config precedence is built-in
  defaults -> `~/.config/snapcode/config.toml` -> `./snapcode.toml` -> flags,
  and each layer overrides only the fields it names (`settings.rs::merge`).
- An `Option` field that can be switched off needs an explicit disabled value in
  TOML, not `None`. TOML has no null, so serde omits a `None` and
  `#[serde(default)]` then restores the *enabled* default on read-back, which
  silently loses the setting. `shadow` and `window.titlebar-height` serialize as
  `false` when off (see `config.rs`'s `disableable_*` modules); any new
  switchable field needs the same treatment.

## Verifying a rendering change

A layout bug still produces a perfectly valid PNG, so `cargo test` alone proves
little. `tests/render.rs` asserts on actual pixels: that geometry lands where
the layout says, that known points carry the theme's colors, and that two
renders are byte-identical. It uses `Renderer::hermetic()`, which skips system
fonts so results never depend on the host — use it for any new rendering test.

When sampling a pixel to check a line decoration, aim between the band's left
edge (`window.x + padding/2`) and where text starts (`window.x + padding`);
anywhere else and you are measuring a glyph or the margin.
