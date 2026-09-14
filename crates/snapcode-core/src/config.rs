// SPDX-License-Identifier: MIT
//! The render configuration: everything that controls how a snippet is drawn.

use std::path::PathBuf;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::color::Rgba;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "kebab-case")]
pub struct RenderConfig {
    pub scale: f32,
    pub dpi: u32,
    pub syntax_theme: String,
    pub theme: String,
    pub font: FontConfig,
    pub window: WindowConfig,
    pub background: Background,
    #[serde(with = "disableable_shadow")]
    pub shadow: Option<ShadowConfig>,
    pub gutter: GutterConfig,
    pub code: CodeConfig,
}

impl Default for RenderConfig {
    fn default() -> Self {
        Self {
            scale: 2.0,
            dpi: 144,
            syntax_theme: "warm".into(),
            theme: "warm".into(),
            font: FontConfig::default(),
            window: WindowConfig::default(),
            background: Background::default(),
            shadow: Some(ShadowConfig::default()),
            gutter: GutterConfig::default(),
            code: CodeConfig::default(),
        }
    }
}

impl RenderConfig {
    pub fn px(&self, logical: f32) -> f32 {
        logical * self.scale
    }

    pub fn validate(&self) -> Result<(), ConfigError> {
        let check = |ok: bool, msg: &'static str| if ok { Ok(()) } else { Err(ConfigError(msg)) };
        check(
            self.scale.is_finite() && (0.1..=10.0).contains(&self.scale),
            "scale must be between 0.1 and 10",
        )?;
        check(
            (1..=2400).contains(&self.dpi),
            "dpi must be between 1 and 2400",
        )?;
        check(
            self.font.size.is_finite() && (1.0..=400.0).contains(&self.font.size),
            "font size must be between 1 and 400",
        )?;
        check(
            self.font.line_height.is_finite() && (0.5..=10.0).contains(&self.font.line_height),
            "line height must be between 0.5 and 10",
        )?;
        check(
            (1..=64).contains(&self.code.tab_width),
            "tab width must be between 1 and 64",
        )?;
        check(
            self.code.dim_amount.is_finite() && (0.0..=1.0).contains(&self.code.dim_amount),
            "dim amount must be between 0 and 1",
        )?;
        check(
            self.window.padding >= 0.0,
            "window padding cannot be negative",
        )?;
        check(
            self.window.margin >= 0.0,
            "window margin cannot be negative",
        )?;
        check(
            self.window.radius >= 0.0,
            "corner radius cannot be negative",
        )?;
        if let Some(w) = self.code.max_width {
            check(w >= 1, "max width must be at least 1 column")?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("invalid configuration: {0}")]
pub struct ConfigError(pub &'static str);

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "kebab-case")]
pub struct FontConfig {
    pub family: Option<String>,
    pub extra_font_files: Vec<PathBuf>,
    pub size: f32,
    pub line_height: f32,
    pub allow_bold: bool,
    pub allow_italic: bool,
    pub ligatures: bool,
}

impl Default for FontConfig {
    fn default() -> Self {
        Self {
            family: None,
            extra_font_files: Vec::new(),
            size: 18.0,
            line_height: 28.0 / 18.0,
            allow_bold: true,
            allow_italic: true,
            ligatures: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "kebab-case")]
pub struct WindowConfig {
    pub padding: f32,
    pub margin: f32,
    pub radius: f32,
    #[serde(with = "disableable_height")]
    pub titlebar_height: Option<f32>,
    pub title: Option<String>,
    pub traffic_lights: TrafficLights,
    pub border: bool,
    pub opaque: bool,
}

impl Default for WindowConfig {
    fn default() -> Self {
        Self {
            padding: 30.0,
            margin: 40.0,
            radius: 10.0,
            titlebar_height: Some(50.0),
            title: None,
            traffic_lights: TrafficLights::Theme,
            border: false,
            opaque: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TrafficLights {
    Macos,
    #[default]
    Theme,
    None,
}

impl FromStr for TrafficLights {
    type Err = ConfigError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "macos" | "mac" => Ok(Self::Macos),
            "theme" => Ok(Self::Theme),
            "none" | "off" => Ok(Self::None),
            _ => Err(ConfigError("traffic lights must be macos, theme, or none")),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
#[derive(Default)]
pub enum Background {
    #[default]
    Theme,
    Solid {
        color: Rgba,
    },
    Linear {
        stops: Vec<GradientStop>,
        angle: f32,
    },
    Radial {
        stops: Vec<GradientStop>,
    },
    Image {
        path: PathBuf,
        #[serde(default)]
        fit: ImageFit,
        #[serde(default)]
        blur: f32,
        #[serde(default)]
        darken: f32,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct GradientStop {
    pub offset: f32,
    pub color: Rgba,
}

impl GradientStop {
    pub fn new(offset: f32, color: Rgba) -> Self {
        Self { offset, color }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ImageFit {
    #[default]
    Fill,
    Fit,
    Tile,
    Center,
    Stretch,
}

impl FromStr for ImageFit {
    type Err = ConfigError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "fill" | "cover" => Ok(Self::Fill),
            "fit" | "contain" => Ok(Self::Fit),
            "tile" | "repeat" => Ok(Self::Tile),
            "center" => Ok(Self::Center),
            "stretch" => Ok(Self::Stretch),
            _ => Err(ConfigError(
                "image fit must be fill, fit, tile, center, or stretch",
            )),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "kebab-case")]
pub struct ShadowConfig {
    pub blur: f32,
    pub offset_x: f32,
    pub offset_y: f32,
    pub color: Rgba,
    pub spread: f32,
}

impl Default for ShadowConfig {
    fn default() -> Self {
        Self {
            blur: 24.0,
            offset_x: 0.0,
            offset_y: 12.0,
            color: Rgba::new(0, 0, 0, 140),
            spread: 0.0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "kebab-case")]
pub struct GutterConfig {
    pub enabled: bool,
    pub start: u32,
    pub padding: f32,
    pub separator: bool,
    pub color: Option<Rgba>,
}

impl Default for GutterConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            start: 1,
            padding: 16.0,
            separator: false,
            color: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "kebab-case")]
pub struct CodeConfig {
    pub language: Option<String>,
    pub tab_width: usize,
    pub max_width: Option<usize>,
    pub wrap: bool,
    pub highlight_lines: Vec<LineRange>,
    pub dim_amount: f32,
    pub diff: bool,
}

impl Default for CodeConfig {
    fn default() -> Self {
        Self {
            language: None,
            tab_width: 4,
            max_width: None,
            wrap: false,
            highlight_lines: Vec::new(),
            dim_amount: 0.55,
            diff: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct LineRange {
    pub start: u32,
    pub end: u32,
}

impl LineRange {
    pub fn new(start: u32, end: u32) -> Self {
        let (start, end) = if start <= end {
            (start, end)
        } else {
            (end, start)
        };
        Self { start, end }
    }

    pub fn contains(&self, line: u32) -> bool {
        (self.start..=self.end).contains(&line)
    }

    pub fn parse_list(spec: &str) -> Result<Vec<LineRange>, ConfigError> {
        let mut out = Vec::new();
        for part in spec.split(',').map(str::trim).filter(|p| !p.is_empty()) {
            let range = match part.split_once('-') {
                Some((a, b)) => {
                    let start = parse_line_no(a)?;
                    let end = parse_line_no(b)?;
                    LineRange::new(start, end)
                }
                None => {
                    let n = parse_line_no(part)?;
                    LineRange::new(n, n)
                }
            };
            out.push(range);
        }
        if out.is_empty() {
            return Err(ConfigError("line range list is empty"));
        }
        Ok(out)
    }
}

fn parse_line_no(s: &str) -> Result<u32, ConfigError> {
    s.trim()
        .parse::<u32>()
        .ok()
        .filter(|n| *n > 0)
        .ok_or(ConfigError("line numbers must be positive integers"))
}

mod disableable_shadow {
    use super::ShadowConfig;
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    #[derive(Serialize, Deserialize)]
    #[serde(untagged)]
    enum Repr {
        Disabled(bool),
        Enabled(ShadowConfig),
    }

    pub fn serialize<S: Serializer>(
        value: &Option<ShadowConfig>,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        match value {
            Some(shadow) => Repr::Enabled(shadow.clone()).serialize(serializer),
            None => Repr::Disabled(false).serialize(serializer),
        }
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Option<ShadowConfig>, D::Error> {
        Ok(match Repr::deserialize(deserializer)? {
            Repr::Disabled(true) => Some(ShadowConfig::default()),
            Repr::Disabled(false) => None,
            Repr::Enabled(shadow) => Some(shadow),
        })
    }
}

mod disableable_height {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    #[derive(Serialize, Deserialize)]
    #[serde(untagged)]
    enum Repr {
        Disabled(bool),
        Height(f32),
    }

    pub fn serialize<S: Serializer>(value: &Option<f32>, serializer: S) -> Result<S::Ok, S::Error> {
        match value {
            Some(height) => Repr::Height(*height).serialize(serializer),
            None => Repr::Disabled(false).serialize(serializer),
        }
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Option<f32>, D::Error> {
        Ok(match Repr::deserialize(deserializer)? {
            Repr::Disabled(true) => Some(50.0),
            Repr::Disabled(false) => None,
            Repr::Height(height) => Some(height),
        })
    }
}

pub fn is_emphasized(ranges: &[LineRange], line: u32) -> bool {
    ranges.is_empty() || ranges.iter().any(|r| r.contains(line))
}

#[cfg(test)]
#[allow(clippy::field_reassign_with_default)]
mod tests {
    use super::*;

    #[test]
    fn default_config_is_valid() {
        RenderConfig::default().validate().unwrap();
    }

    #[test]
    fn rejects_out_of_range_values() {
        let mut c = RenderConfig::default();
        c.scale = 0.0;
        assert!(c.validate().is_err());

        let mut c = RenderConfig::default();
        c.code.tab_width = 0;
        assert!(c.validate().is_err());

        let mut c = RenderConfig::default();
        c.font.size = f32::NAN;
        assert!(c.validate().is_err());
    }

    #[test]
    fn scales_logical_units() {
        let mut c = RenderConfig::default();
        c.scale = 2.0;
        assert_eq!(c.px(30.0), 60.0);
        c.scale = 1.0;
        assert_eq!(c.px(30.0), 30.0);
    }

    #[test]
    fn parses_line_range_specs() {
        assert_eq!(
            LineRange::parse_list("3").unwrap(),
            vec![LineRange::new(3, 3)]
        );
        assert_eq!(
            LineRange::parse_list("1,4-6, 9").unwrap(),
            vec![
                LineRange::new(1, 1),
                LineRange::new(4, 6),
                LineRange::new(9, 9)
            ]
        );
        assert_eq!(
            LineRange::parse_list("6-4").unwrap(),
            vec![LineRange::new(4, 6)]
        );
        for bad in ["", "0", "a-b", "1-", "-"] {
            assert!(LineRange::parse_list(bad).is_err(), "{bad:?} should fail");
        }
    }

    #[test]
    fn emphasis_defaults_to_every_line() {
        assert!(is_emphasized(&[], 1));
        assert!(is_emphasized(&[], 999));

        let ranges = LineRange::parse_list("2-4").unwrap();
        assert!(!is_emphasized(&ranges, 1));
        assert!(is_emphasized(&ranges, 3));
        assert!(!is_emphasized(&ranges, 5));
    }

    #[test]
    fn roundtrips_through_toml() {
        let original = RenderConfig::default();
        let text = toml::to_string(&original).unwrap();
        let parsed: RenderConfig = toml::from_str(&text).unwrap();
        assert_eq!(original, parsed);
    }

    #[test]
    fn partial_toml_fills_in_defaults() {
        let parsed: RenderConfig = toml::from_str("scale = 3.0\n[font]\nsize = 24.0\n").unwrap();
        assert_eq!(parsed.scale, 3.0);
        assert_eq!(parsed.font.size, 24.0);
        assert_eq!(parsed.dpi, RenderConfig::default().dpi);
        assert_eq!(parsed.window.padding, 30.0);
    }

    #[test]
    fn a_disabled_shadow_and_titlebar_survive_a_round_trip() {
        let mut config = RenderConfig::default();
        config.shadow = None;
        config.window.titlebar_height = None;

        let text = toml::to_string(&config).unwrap();
        assert!(text.contains("shadow = false"), "{text}");
        assert!(text.contains("titlebar-height = false"), "{text}");

        let parsed: RenderConfig = toml::from_str(&text).unwrap();
        assert_eq!(parsed.shadow, None, "a saved 'off' came back enabled");
        assert_eq!(parsed.window.titlebar_height, None);
        assert_eq!(parsed, config);
    }

    #[test]
    fn an_enabled_shadow_and_titlebar_still_round_trip() {
        let config = RenderConfig::default();
        let parsed: RenderConfig = toml::from_str(&toml::to_string(&config).unwrap()).unwrap();
        assert_eq!(parsed, config);
        assert!(parsed.shadow.is_some());
        assert_eq!(parsed.window.titlebar_height, Some(50.0));
    }

    #[test]
    fn true_is_accepted_as_shorthand_for_the_default() {
        let parsed: RenderConfig =
            toml::from_str("shadow = true\n[window]\ntitlebar-height = true\n").unwrap();
        assert_eq!(parsed.shadow, Some(ShadowConfig::default()));
        assert_eq!(parsed.window.titlebar_height, Some(50.0));
    }

    #[test]
    fn rejects_unknown_keys() {
        assert!(toml::from_str::<RenderConfig>("scal = 3.0").is_err());
    }
}
