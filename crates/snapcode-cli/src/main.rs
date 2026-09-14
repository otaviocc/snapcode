// SPDX-License-Identifier: MIT
//! `snapcode` — render code snippets to images, from the shell or a TUI.

mod cli;
mod io;
mod settings;
mod tui;

use std::path::Path;

use anyhow::{Context, Result};
use clap::{CommandFactory, Parser};
use snapcode_core::config::RenderConfig;
use snapcode_core::{RenderRequest, Renderer};

use crate::cli::{Cli, Command, ConfigAction, Format, RenderArgs};
use crate::io::Destination;

fn main() -> std::process::ExitCode {
    let cli = Cli::parse();
    match run(cli) {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("snapcode: {error:#}");
            std::process::ExitCode::FAILURE
        }
    }
}

fn run(cli: Cli) -> Result<()> {
    match cli.command {
        None => render(&cli.render),
        Some(Command::Tui(args)) => {
            let (config, renderer, input) = prepare(&args)?;
            tui::run(config, renderer, input, &args)
        }
        Some(Command::Themes) => list_themes(&cli.render),
        Some(Command::Languages) => list_languages(),
        Some(Command::Config { action }) => configure(action, &cli.render),
        Some(Command::Completions { shell }) => {
            let mut command = Cli::command();
            let name = command.get_name().to_string();
            clap_complete::generate(shell, &mut command, name, &mut std::io::stdout());
            Ok(())
        }
    }
}

fn prepare(args: &RenderArgs) -> Result<(RenderConfig, Renderer, io::Input)> {
    let mut config = settings::load(args.config.as_deref(), args.no_config)?;
    args.apply(&mut config)?;

    let mut renderer = Renderer::new();
    if let Some(dir) = settings::themes_dir() {
        renderer
            .load_themes(&dir)
            .with_context(|| format!("failed to load themes from {}", dir.display()))?;
    }

    let input = io::read_input(args.input.as_deref())?;
    Ok((config, renderer, input))
}

fn render(args: &RenderArgs) -> Result<()> {
    let (config, renderer, input) = prepare(args)?;
    render_once(&config, &renderer, &input, args)?;

    if args.watch {
        let path = input
            .path
            .clone()
            .context("--watch needs a file to watch; it cannot watch stdin or the clipboard")?;
        watch(&path, &config, &renderer, args)?;
    }
    Ok(())
}

fn render_once(
    config: &RenderConfig,
    renderer: &Renderer,
    input: &io::Input,
    args: &RenderArgs,
) -> Result<()> {
    let format = resolve_format(args);
    let destination = io::resolve_destination(
        args.output.as_deref(),
        input.path.as_deref(),
        match format {
            Format::Png => "png",
            Format::Svg => "svg",
        },
    );

    let mut request = RenderRequest::new(&input.source, config);
    if let Some(path) = &input.path {
        request = request.with_path(path.clone());
    }

    match format {
        Format::Png => {
            let raster = renderer.render_raster(&request)?;
            let png = snapcode_core::encode::to_png(&raster, config.dpi)?;
            io::write_output(&destination, &png)?;
            if args.copy {
                io::copy_image_to_clipboard(raster.width, raster.height, &raster.pixels)?;
            }
            report(
                &destination,
                raster.width,
                raster.height,
                png.len(),
                args.copy,
            );
        }
        Format::Svg => {
            let (svg, dropped) = renderer.render_svg(&request)?;
            if dropped {
                eprintln!(
                    "snapcode: note: image backgrounds are not embedded in SVG; \
                     the background was left blank"
                );
            }
            io::write_output(&destination, svg.as_bytes())?;
            if args.copy {
                io::copy_text_to_clipboard(&svg)?;
            }
            if let Destination::File(path) = &destination {
                eprintln!(
                    "snapcode: wrote {} ({})",
                    path.display(),
                    human_size(svg.len())
                );
            }
        }
    }
    Ok(())
}

fn resolve_format(args: &RenderArgs) -> Format {
    if let Some(format) = args.format {
        return format;
    }
    let extension = args
        .output
        .as_deref()
        .and_then(|o| Path::new(o).extension())
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase);
    match extension.as_deref() {
        Some("svg") => Format::Svg,
        _ => Format::Png,
    }
}

fn report(destination: &Destination, width: u32, height: u32, bytes: usize, copied: bool) {
    if let Destination::File(path) = destination {
        eprintln!(
            "snapcode: wrote {} ({width}x{height}, {})",
            path.display(),
            human_size(bytes)
        );
    }
    if copied {
        eprintln!("snapcode: copied to the clipboard");
    }
}

fn human_size(bytes: usize) -> String {
    const KIB: f64 = 1024.0;
    let bytes = bytes as f64;
    if bytes < KIB {
        format!("{bytes:.0} B")
    } else if bytes < KIB * KIB {
        format!("{:.1} KiB", bytes / KIB)
    } else {
        format!("{:.1} MiB", bytes / (KIB * KIB))
    }
}

fn watch(path: &Path, config: &RenderConfig, renderer: &Renderer, args: &RenderArgs) -> Result<()> {
    use notify::Watcher;

    let (tx, rx) = std::sync::mpsc::channel();
    let mut watcher = notify::recommended_watcher(move |event| {
        let _ = tx.send(event);
    })
    .context("failed to start the file watcher")?;

    let directory = path.parent().filter(|p| !p.as_os_str().is_empty());
    match directory {
        Some(dir) => watcher.watch(dir, notify::RecursiveMode::NonRecursive),
        None => watcher.watch(Path::new("."), notify::RecursiveMode::NonRecursive),
    }
    .with_context(|| format!("failed to watch {}", path.display()))?;

    let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    eprintln!("snapcode: watching {} (ctrl-c to stop)", path.display());

    for event in rx {
        let Ok(event) = event else { continue };
        let touched = event.paths.iter().any(|p| {
            p.canonicalize()
                .map(|p| p == canonical)
                .unwrap_or_else(|_| p == path)
        });
        if !touched || !event.kind.is_modify() && !event.kind.is_create() {
            continue;
        }
        let Ok(source) = std::fs::read_to_string(path) else {
            continue;
        };
        let input = io::Input {
            source,
            path: Some(path.to_path_buf()),
        };
        if let Err(error) = render_once(config, renderer, &input, args) {
            eprintln!("snapcode: {error:#}");
        }
    }
    Ok(())
}

fn list_themes(args: &RenderArgs) -> Result<()> {
    let mut renderer = Renderer::new();
    if let Some(dir) = settings::themes_dir() {
        renderer.load_themes(&dir)?;
    }
    let _ = args;

    println!("Chrome themes (--theme):");
    for name in renderer.themes().chrome_names() {
        println!("  {name}");
    }
    println!("\nSyntax themes (--syntax-theme):");
    for name in renderer.themes().syntax_names() {
        println!("  {name}");
    }
    if let Some(dir) = settings::themes_dir() {
        println!("\nUser themes are loaded from {}", dir.display());
    }
    Ok(())
}

fn list_languages() -> Result<()> {
    let renderer = Renderer::new();
    for name in renderer.highlighter().language_names() {
        println!("{name}");
    }
    Ok(())
}

fn effective_config(outer: &RenderArgs, inner: &RenderArgs) -> Result<RenderConfig> {
    let config_path = inner.config.as_deref().or(outer.config.as_deref());
    let mut config = settings::load(config_path, outer.no_config || inner.no_config)?;
    outer.apply(&mut config)?;
    inner.apply(&mut config)?;
    Ok(config)
}

fn configure(action: ConfigAction, args: &RenderArgs) -> Result<()> {
    match action {
        ConfigAction::Path => {
            let path =
                settings::config_path().context("could not determine a configuration directory")?;
            println!("{}", path.display());
            Ok(())
        }
        ConfigAction::Print(sub) => {
            let config = effective_config(args, &sub)?;
            print!("{}", settings::to_toml(&config)?);
            Ok(())
        }
        ConfigAction::Save(sub) => {
            let config = effective_config(args, &sub)?;
            let path = settings::save(&config)?;
            eprintln!("snapcode: wrote {}", path.display());
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_comes_from_the_flag_first() {
        let args = RenderArgs {
            format: Some(Format::Svg),
            output: Some("out.png".into()),
            ..Default::default()
        };
        assert_eq!(resolve_format(&args), Format::Svg);
    }

    #[test]
    fn format_falls_back_to_the_output_extension() {
        let svg = RenderArgs {
            output: Some("out.svg".into()),
            ..Default::default()
        };
        assert_eq!(resolve_format(&svg), Format::Svg);

        let upper = RenderArgs {
            output: Some("out.SVG".into()),
            ..Default::default()
        };
        assert_eq!(resolve_format(&upper), Format::Svg);
    }

    #[test]
    fn format_defaults_to_png() {
        assert_eq!(resolve_format(&RenderArgs::default()), Format::Png);
        let odd = RenderArgs {
            output: Some("out.jpeg".into()),
            ..Default::default()
        };
        assert_eq!(resolve_format(&odd), Format::Png);
        let stdout = RenderArgs {
            output: Some("-".into()),
            ..Default::default()
        };
        assert_eq!(resolve_format(&stdout), Format::Png);
    }

    #[test]
    fn sizes_are_human_readable() {
        assert_eq!(human_size(512), "512 B");
        assert_eq!(human_size(1536), "1.5 KiB");
        assert_eq!(human_size(2 * 1024 * 1024), "2.0 MiB");
    }
}
