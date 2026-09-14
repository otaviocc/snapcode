// SPDX-License-Identifier: MIT
//! Command-line interface definition and configuration layering.

use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};
use snapcode_core::color::Rgba;
use snapcode_core::config::{
    Background, GradientStop, ImageFit, LineRange, RenderConfig, TrafficLights,
};

#[derive(Debug, Parser)]
#[command(name = "snapcode", version, about = "Render code snippets to images.", long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,

    #[command(flatten)]
    pub render: RenderArgs,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    #[command(about = "Open the interactive TUI with a live preview.")]
    Tui(RenderArgs),
    #[command(about = "List available chrome and syntax themes.")]
    Themes,
    #[command(about = "List available languages.")]
    Languages,
    #[command(about = "Inspect or write the configuration file.")]
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
    #[command(about = "Print a shell completion script.")]
    Completions {
        #[arg(value_enum, help = "Shell to generate completions for")]
        shell: clap_complete::Shell,
    },
}

#[derive(Debug, Subcommand)]
pub enum ConfigAction {
    #[command(about = "Print the path snapcode reads its configuration from.")]
    Path,
    #[command(about = "Print the effective configuration as TOML.")]
    Print(#[command(flatten)] RenderArgs),
    #[command(about = "Write the effective configuration to the config file.")]
    Save(#[command(flatten)] RenderArgs),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum Format {
    Png,
    Svg,
}

#[derive(Debug, Clone, Default, Args)]
pub struct RenderArgs {
    #[arg(help = "Input file, or `-` for stdin. Omit to read the clipboard")]
    pub input: Option<String>,

    #[arg(
        short,
        long,
        help = "Output path, or `-` for stdout. Defaults to the input name"
    )]
    pub output: Option<String>,

    #[arg(
        long,
        value_enum,
        help = "Output format. Inferred from the output extension when not given"
    )]
    pub format: Option<Format>,

    #[arg(long, help = "Copy the rendered image to the clipboard")]
    pub copy: bool,

    #[arg(long, help = "Re-render whenever the input file changes")]
    pub watch: bool,

    #[arg(
        short,
        long,
        help = "Language override; otherwise detected from the filename or content"
    )]
    pub lang: Option<String>,

    #[arg(long, help = "Chrome theme (window, titlebar, background)")]
    pub theme: Option<String>,

    #[arg(long, help = "Syntax theme")]
    pub syntax_theme: Option<String>,

    #[arg(long, help = "Font family. Defaults to the embedded JetBrains Mono")]
    pub font: Option<String>,

    #[arg(long, help = "Font size in logical units")]
    pub font_size: Option<f32>,

    #[arg(long, help = "Line height as a multiple of font size")]
    pub line_height: Option<f32>,

    #[arg(long, help = "Enable programming ligatures")]
    pub ligatures: bool,

    #[arg(long, help = "Pixel multiplier: 1 normal, 2 retina, 3+ print")]
    pub scale: Option<f32>,

    #[arg(long, help = "DPI metadata written into the PNG")]
    pub dpi: Option<u32>,

    #[arg(
        long,
        help = "A color, `linear:<angle>:<c1>,<c2>`, `radial:<c1>,<c2>`, or an image path"
    )]
    pub bg: Option<String>,

    #[arg(
        long,
        value_enum,
        help = "How an image background is mapped onto the canvas"
    )]
    pub bg_fit: Option<BgFit>,

    #[arg(long, help = "Blur radius for an image background")]
    pub bg_blur: Option<f32>,

    #[arg(
        long,
        allow_negative_numbers = true,
        help = "Darken (positive) or brighten (negative) an image background, -1 to 1"
    )]
    pub bg_darken: Option<f32>,

    #[arg(long, help = "Space between the code and the window edge")]
    pub padding: Option<f32>,

    #[arg(long, help = "Space between the window and the image edge")]
    pub margin: Option<f32>,

    #[arg(long, help = "Window corner radius")]
    pub radius: Option<f32>,

    #[arg(long, help = "Draw a hairline border around the window")]
    pub border: bool,

    #[arg(long, conflicts_with = "title", help = "Hide the titlebar")]
    pub no_titlebar: bool,

    #[arg(long, help = "Titlebar text. Defaults to the input filename")]
    pub title: Option<String>,

    #[arg(long, value_enum, help = "Style of the three dots in the titlebar")]
    pub traffic_lights: Option<Lights>,

    #[arg(long, conflicts_with_all = ["shadow_blur", "shadow_color"], help = "Disable the drop shadow")]
    pub no_shadow: bool,

    #[arg(long, help = "Drop shadow blur radius")]
    pub shadow_blur: Option<f32>,

    #[arg(long, help = "Drop shadow color")]
    pub shadow_color: Option<String>,

    #[arg(long, help = "Show line numbers")]
    pub line_numbers: bool,

    #[arg(long, help = "Number shown on the first line")]
    pub line_start: Option<u32>,

    #[arg(long, help = "Draw a rule between the gutter and the code")]
    pub gutter_separator: bool,

    #[arg(long, help = "Emphasize these lines and dim the rest, e.g. `3-5,9`")]
    pub highlight_lines: Option<String>,

    #[arg(long, help = "Render leading +/- as diff line backgrounds")]
    pub diff: bool,

    #[arg(long, help = "Spaces a tab expands to")]
    pub tab_width: Option<usize>,

    #[arg(long, help = "Maximum width in columns")]
    pub max_width: Option<usize>,

    #[arg(
        long,
        requires = "max_width",
        help = "Wrap at --max-width instead of clipping"
    )]
    pub wrap: bool,

    #[arg(
        long,
        conflicts_with = "no_config",
        help = "Use a specific configuration file"
    )]
    pub config: Option<PathBuf>,

    #[arg(long, help = "Ignore configuration files entirely")]
    pub no_config: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum BgFit {
    Fill,
    Fit,
    Tile,
    Center,
    Stretch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum Lights {
    Macos,
    Theme,
    None,
}

#[derive(Debug, thiserror::Error)]
pub enum ArgError {
    #[error("invalid color {value:?}: {reason}")]
    Color { value: String, reason: String },
    #[error("invalid background {value:?}: {reason}")]
    Background { value: String, reason: &'static str },
    #[error("invalid line range {value:?}: {reason}")]
    LineRange { value: String, reason: String },
}

impl RenderArgs {
    pub fn apply(&self, config: &mut RenderConfig) -> Result<(), ArgError> {
        if let Some(v) = self.scale {
            config.scale = v;
        }
        if let Some(v) = self.dpi {
            config.dpi = v;
        }
        if let Some(v) = &self.theme {
            config.theme = v.clone();
        }
        if let Some(v) = &self.syntax_theme {
            config.syntax_theme = v.clone();
        }

        if let Some(v) = &self.font {
            config.font.family = Some(v.clone());
        }
        if let Some(v) = self.font_size {
            config.font.size = v;
        }
        if let Some(v) = self.line_height {
            config.font.line_height = v;
        }
        if self.ligatures {
            config.font.ligatures = true;
        }

        if let Some(v) = self.padding {
            config.window.padding = v;
        }
        if let Some(v) = self.margin {
            config.window.margin = v;
        }
        if let Some(v) = self.radius {
            config.window.radius = v;
        }
        if self.border {
            config.window.border = true;
        }
        if self.no_titlebar {
            config.window.titlebar_height = None;
        }
        if let Some(v) = &self.title {
            config.window.title = Some(v.clone());
        }
        if let Some(v) = self.traffic_lights {
            config.window.traffic_lights = match v {
                Lights::Macos => TrafficLights::Macos,
                Lights::Theme => TrafficLights::Theme,
                Lights::None => TrafficLights::None,
            };
        }

        if let Some(spec) = &self.bg {
            config.background = parse_background(spec, self.bg_fit, self.bg_blur, self.bg_darken)?;
        } else if let Background::Image {
            path,
            fit,
            blur,
            darken,
        } = &config.background
        {
            config.background = Background::Image {
                path: path.clone(),
                fit: self.bg_fit.map(image_fit_of).unwrap_or(*fit),
                blur: self.bg_blur.unwrap_or(*blur),
                darken: self.bg_darken.unwrap_or(*darken),
            };
        }

        if self.no_shadow {
            config.shadow = None;
        } else if self.shadow_blur.is_some() || self.shadow_color.is_some() {
            let mut shadow = config.shadow.clone().unwrap_or_default();
            if let Some(v) = self.shadow_blur {
                shadow.blur = v;
            }
            if let Some(v) = &self.shadow_color {
                shadow.color = parse_color(v)?;
            }
            config.shadow = Some(shadow);
        }

        if self.line_numbers {
            config.gutter.enabled = true;
        }
        if let Some(v) = self.line_start {
            config.gutter.enabled = true;
            config.gutter.start = v;
        }
        if self.gutter_separator {
            config.gutter.enabled = true;
            config.gutter.separator = true;
        }

        if let Some(v) = &self.lang {
            config.code.language = Some(v.clone());
        }
        if let Some(v) = self.tab_width {
            config.code.tab_width = v;
        }
        if let Some(v) = self.max_width {
            config.code.max_width = Some(v);
        }
        if self.wrap {
            config.code.wrap = true;
        }
        if self.diff {
            config.code.diff = true;
        }
        if let Some(spec) = &self.highlight_lines {
            config.code.highlight_lines =
                LineRange::parse_list(spec).map_err(|e| ArgError::LineRange {
                    value: spec.clone(),
                    reason: e.0.to_string(),
                })?;
        }

        Ok(())
    }
}

fn image_fit_of(fit: BgFit) -> ImageFit {
    match fit {
        BgFit::Fill => ImageFit::Fill,
        BgFit::Fit => ImageFit::Fit,
        BgFit::Tile => ImageFit::Tile,
        BgFit::Center => ImageFit::Center,
        BgFit::Stretch => ImageFit::Stretch,
    }
}

fn parse_color(value: &str) -> Result<Rgba, ArgError> {
    value
        .parse()
        .map_err(|e: snapcode_core::color::ParseColorError| ArgError::Color {
            value: value.to_string(),
            reason: e.reason.to_string(),
        })
}

pub fn parse_background(
    spec: &str,
    fit: Option<BgFit>,
    blur: Option<f32>,
    darken: Option<f32>,
) -> Result<Background, ArgError> {
    let spec = spec.trim();
    let bad = |reason: &'static str| ArgError::Background {
        value: spec.to_string(),
        reason,
    };

    if let Some(rest) = spec.strip_prefix("linear:") {
        let (angle, colors) = rest
            .split_once(':')
            .ok_or_else(|| bad("expected linear:<angle>:<colors>"))?;
        let angle: f32 = angle
            .trim()
            .parse()
            .map_err(|_| bad("angle must be a number in degrees"))?;
        return Ok(Background::Linear {
            stops: parse_stops(colors, spec)?,
            angle,
        });
    }
    if let Some(colors) = spec.strip_prefix("radial:") {
        return Ok(Background::Radial {
            stops: parse_stops(colors, spec)?,
        });
    }

    if let Ok(color) = spec.parse::<Rgba>() {
        return Ok(Background::Solid { color });
    }

    let path = PathBuf::from(spec);
    if path.exists() {
        return Ok(Background::Image {
            path,
            fit: image_fit_of(fit.unwrap_or(BgFit::Fill)),
            blur: blur.unwrap_or(0.0),
            darken: darken.unwrap_or(0.0),
        });
    }

    Err(bad(
        "not a color, a gradient spec, or an existing image path",
    ))
}

fn parse_stops(list: &str, spec: &str) -> Result<Vec<GradientStop>, ArgError> {
    let colors: Vec<&str> = list
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect();
    if colors.len() < 2 {
        return Err(ArgError::Background {
            value: spec.to_string(),
            reason: "a gradient needs at least two colors",
        });
    }
    let last = colors.len() - 1;
    colors
        .iter()
        .enumerate()
        .map(|(i, c)| Ok(GradientStop::new(i as f32 / last as f32, parse_color(c)?)))
        .collect()
}

#[cfg(test)]
#[allow(clippy::field_reassign_with_default)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn cli_definition_is_valid() {
        Cli::command().debug_assert();
    }

    #[test]
    fn every_argument_and_subcommand_documents_itself() {
        fn check(command: &clap::Command, path: &str) {
            for arg in command.get_arguments() {
                if arg.get_id() == "help" || arg.get_id() == "version" {
                    continue;
                }
                assert!(
                    arg.get_help().is_some(),
                    "{path} --{} has no help text",
                    arg.get_id()
                );
            }
            for sub in command.get_subcommands() {
                if sub.get_name() == "help" {
                    continue;
                }
                assert!(
                    sub.get_about().is_some(),
                    "{path} {} has no about text",
                    sub.get_name()
                );
                check(sub, &format!("{path} {}", sub.get_name()));
            }
        }

        let command = Cli::command();
        assert!(
            command.get_about().is_some(),
            "the binary has no about text"
        );
        check(&command, "snapcode");
    }

    #[test]
    fn unset_flags_leave_the_config_alone() {
        let original = RenderConfig::default();
        let mut config = original.clone();
        RenderArgs::default().apply(&mut config).unwrap();
        assert_eq!(config, original, "a bare invocation must change nothing");
    }

    #[test]
    fn flags_override_individual_values() {
        let mut config = RenderConfig::default();
        let args = RenderArgs {
            scale: Some(3.0),
            font_size: Some(22.0),
            theme: Some("midnight".into()),
            padding: Some(12.0),
            border: true,
            no_titlebar: true,
            ..Default::default()
        };
        args.apply(&mut config).unwrap();

        assert_eq!(config.scale, 3.0);
        assert_eq!(config.font.size, 22.0);
        assert_eq!(config.theme, "midnight");
        assert_eq!(config.window.padding, 12.0);
        assert!(config.window.border);
        assert_eq!(config.window.titlebar_height, None);
        assert_eq!(config.dpi, RenderConfig::default().dpi);
    }

    #[test]
    fn gutter_flags_imply_line_numbers() {
        let mut config = RenderConfig::default();
        assert!(!config.gutter.enabled);
        RenderArgs {
            line_start: Some(10),
            ..Default::default()
        }
        .apply(&mut config)
        .unwrap();
        assert!(
            config.gutter.enabled,
            "--line-start should turn the gutter on"
        );
        assert_eq!(config.gutter.start, 10);
    }

    #[test]
    fn shadow_flags_modify_rather_than_replace() {
        let mut config = RenderConfig::default();
        let original_offset = config.shadow.as_ref().unwrap().offset_y;
        RenderArgs {
            shadow_blur: Some(50.0),
            ..Default::default()
        }
        .apply(&mut config)
        .unwrap();
        let shadow = config.shadow.as_ref().unwrap();
        assert_eq!(shadow.blur, 50.0);
        assert_eq!(shadow.offset_y, original_offset, "offset should survive");

        let mut config = RenderConfig::default();
        RenderArgs {
            no_shadow: true,
            ..Default::default()
        }
        .apply(&mut config)
        .unwrap();
        assert!(config.shadow.is_none());
    }

    #[test]
    fn bg_modifiers_apply_to_an_inherited_image_background() {
        let mut config = RenderConfig::default();
        config.background = Background::Image {
            path: PathBuf::from("bg.png"),
            fit: ImageFit::Fill,
            blur: 0.0,
            darken: 0.0,
        };
        RenderArgs {
            bg_blur: Some(20.0),
            bg_darken: Some(0.4),
            bg_fit: Some(BgFit::Tile),
            ..Default::default()
        }
        .apply(&mut config)
        .unwrap();

        match config.background {
            Background::Image {
                fit, blur, darken, ..
            } => {
                assert_eq!(blur, 20.0, "--bg-blur was dropped");
                assert_eq!(darken, 0.4);
                assert_eq!(fit, ImageFit::Tile);
            }
            other => panic!("expected an image background, got {other:?}"),
        }
    }

    #[test]
    fn bg_modifiers_do_not_disturb_a_non_image_background() {
        let mut config = RenderConfig::default();
        let before = config.background.clone();
        RenderArgs {
            bg_blur: Some(20.0),
            ..Default::default()
        }
        .apply(&mut config)
        .unwrap();
        assert_eq!(config.background, before);
    }

    #[test]
    fn parses_solid_color_backgrounds() {
        let bg = parse_background("#C15F3C", None, None, None).unwrap();
        assert_eq!(
            bg,
            Background::Solid {
                color: Rgba::rgb(0xC1, 0x5F, 0x3C)
            }
        );
        assert!(matches!(
            parse_background("rebeccapurple-nope", None, None, None),
            Err(ArgError::Background { .. })
        ));
    }

    #[test]
    fn parses_gradient_backgrounds() {
        let bg = parse_background("linear:135:#000,#fff", None, None, None).unwrap();
        match bg {
            Background::Linear { stops, angle } => {
                assert_eq!(angle, 135.0);
                assert_eq!(stops.len(), 2);
                assert_eq!(stops[0].offset, 0.0);
                assert_eq!(stops[1].offset, 1.0);
                assert_eq!(stops[1].color, Rgba::WHITE);
            }
            other => panic!("expected a linear gradient, got {other:?}"),
        }

        let bg = parse_background("radial:#000,#888,#fff", None, None, None).unwrap();
        match bg {
            Background::Radial { stops } => {
                assert_eq!(stops.len(), 3);
                assert_eq!(stops[1].offset, 0.5);
            }
            other => panic!("expected a radial gradient, got {other:?}"),
        }
    }

    #[test]
    fn rejects_malformed_gradients() {
        for bad in [
            "linear:135",
            "linear:abc:#000,#fff",
            "radial:#000",
            "linear:90:#000",
        ] {
            assert!(
                parse_background(bad, None, None, None).is_err(),
                "{bad:?} should not parse"
            );
        }
    }

    #[test]
    fn parses_highlight_ranges() {
        let mut config = RenderConfig::default();
        RenderArgs {
            highlight_lines: Some("2-4,8".into()),
            ..Default::default()
        }
        .apply(&mut config)
        .unwrap();
        assert_eq!(config.code.highlight_lines.len(), 2);

        let mut config = RenderConfig::default();
        assert!(matches!(
            RenderArgs {
                highlight_lines: Some("nope".into()),
                ..Default::default()
            }
            .apply(&mut config),
            Err(ArgError::LineRange { .. })
        ));
    }

    #[test]
    fn parses_a_full_command_line() {
        let cli = Cli::try_parse_from([
            "snapcode",
            "main.swift",
            "-o",
            "out.png",
            "--scale",
            "3",
            "--line-numbers",
            "--highlight-lines",
            "2-4",
            "--bg",
            "linear:90:#000,#fff",
        ])
        .unwrap();
        assert_eq!(cli.render.input.as_deref(), Some("main.swift"));
        assert_eq!(cli.render.output.as_deref(), Some("out.png"));
        assert_eq!(cli.render.scale, Some(3.0));
        assert!(cli.render.line_numbers);

        let mut config = RenderConfig::default();
        cli.render.apply(&mut config).unwrap();
        assert_eq!(config.scale, 3.0);
        assert!(config.gutter.enabled);
        assert_eq!(config.code.highlight_lines.len(), 1);
    }

    #[test]
    fn wrap_requires_max_width() {
        assert!(Cli::try_parse_from(["snapcode", "a.rs", "--wrap"]).is_err());
        assert!(Cli::try_parse_from(["snapcode", "a.rs", "--wrap", "--max-width", "80"]).is_ok());
    }

    #[test]
    fn conflicting_flags_are_rejected() {
        assert!(
            Cli::try_parse_from(["snapcode", "a.rs", "--no-shadow", "--shadow-blur", "10"])
                .is_err()
        );
        assert!(
            Cli::try_parse_from(["snapcode", "a.rs", "--no-titlebar", "--title", "x"]).is_err()
        );
        assert!(
            Cli::try_parse_from(["snapcode", "a.rs", "--no-config", "--config", "c.toml"]).is_err()
        );
    }

    #[test]
    fn negative_darken_is_accepted() {
        let cli = Cli::try_parse_from(["snapcode", "a.rs", "--bg-darken", "-0.3"]).unwrap();
        assert_eq!(cli.render.bg_darken, Some(-0.3));
    }
}
