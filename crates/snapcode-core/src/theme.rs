// SPDX-License-Identifier: MIT
//! Chrome themes and the registry that resolves a name to a chrome and a syntax theme.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::color::Rgba;
use crate::config::Background;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "kebab-case")]
pub struct ChromeTheme {
    pub name: String,
    pub syntax_theme: String,
    pub window: Option<Rgba>,
    pub titlebar: Option<Rgba>,
    pub border: Rgba,
    pub gutter: Rgba,
    pub gutter_separator: Rgba,
    pub title_text: Rgba,
    pub traffic_lights: [Rgba; 3],
    pub shadow: Rgba,
    pub line_emphasis: Rgba,
    pub diff_added: Rgba,
    pub diff_removed: Rgba,
    pub background: Background,
}

pub const MACOS_TRAFFIC_LIGHTS: [Rgba; 3] = [
    Rgba::rgb(0xff, 0x5f, 0x57),
    Rgba::rgb(0xfe, 0xbc, 0x2e),
    Rgba::rgb(0x28, 0xc8, 0x40),
];

impl Default for ChromeTheme {
    fn default() -> Self {
        Self {
            name: "default".into(),
            syntax_theme: "warm".into(),
            window: None,
            titlebar: None,
            border: Rgba::new(255, 255, 255, 20),
            gutter: Rgba::new(255, 255, 255, 90),
            gutter_separator: Rgba::new(255, 255, 255, 18),
            title_text: Rgba::new(255, 255, 255, 140),
            traffic_lights: MACOS_TRAFFIC_LIGHTS,
            shadow: Rgba::new(0, 0, 0, 150),
            line_emphasis: Rgba::new(255, 255, 255, 13),
            diff_added: Rgba::new(0x28, 0xc8, 0x40, 38),
            diff_removed: Rgba::new(0xff, 0x5f, 0x57, 38),
            background: Background::Solid {
                color: Rgba::rgb(0x2d, 0x2d, 0x2d),
            },
        }
    }
}

impl ChromeTheme {
    pub fn window_color(&self, syntax_background: Rgba) -> Rgba {
        self.window.unwrap_or(syntax_background)
    }

    pub fn titlebar_color(&self, syntax_background: Rgba) -> Rgba {
        self.titlebar.unwrap_or_else(|| {
            let base = self.window_color(syntax_background);
            let target = if base.is_dark() {
                Rgba::WHITE
            } else {
                Rgba::BLACK
            };
            base.mix_rgb(target, 0.07)
        })
    }
}

const ANSI_PALETTE: [Rgba; 16] = [
    Rgba::rgb(0x00, 0x00, 0x00),
    Rgba::rgb(0xcd, 0x31, 0x31),
    Rgba::rgb(0x0d, 0xbc, 0x79),
    Rgba::rgb(0xe5, 0xe5, 0x10),
    Rgba::rgb(0x24, 0x72, 0xc8),
    Rgba::rgb(0xbc, 0x3f, 0xbc),
    Rgba::rgb(0x11, 0xa8, 0xcd),
    Rgba::rgb(0xe5, 0xe5, 0xe5),
    Rgba::rgb(0x66, 0x66, 0x66),
    Rgba::rgb(0xf1, 0x4c, 0x4c),
    Rgba::rgb(0x23, 0xd1, 0x8b),
    Rgba::rgb(0xf5, 0xf5, 0x43),
    Rgba::rgb(0x3b, 0x8e, 0xea),
    Rgba::rgb(0xd6, 0x70, 0xd6),
    Rgba::rgb(0x29, 0xb8, 0xdb),
    Rgba::rgb(0xff, 0xff, 0xff),
];

pub fn resolve_color(color: syntect::highlighting::Color, default: Rgba) -> Rgba {
    match color.a {
        0 => ANSI_PALETTE[(color.r as usize).min(ANSI_PALETTE.len() - 1)],
        1 => default,
        _ => Rgba::new(color.r, color.g, color.b, color.a),
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SyntaxAccents {
    pub background: Rgba,
    pub foreground: Rgba,
    pub keyword: Rgba,
    pub string: Rgba,
    pub comment: Rgba,
    pub function: Rgba,
}

pub fn accents_of(theme: &syntect::highlighting::Theme) -> SyntaxAccents {
    use syntect::highlighting::Highlighter;
    use syntect::parsing::Scope;

    let highlighter = Highlighter::new(theme);
    let default = highlighter.get_default();
    let background = resolve_color(default.background, Rgba::rgb(0x1a, 0x1a, 0x1a));
    let foreground = resolve_color(default.foreground, Rgba::rgb(0xd0, 0xd0, 0xd0));

    let foreground = foreground.over(background);
    let color_of = |scope: &str| {
        Scope::new(scope)
            .ok()
            .map(|scope| {
                resolve_color(highlighter.style_for_stack(&[scope]).foreground, foreground)
                    .over(background)
            })
            .unwrap_or(foreground)
    };

    SyntaxAccents {
        background,
        foreground,
        keyword: color_of("keyword.control"),
        string: color_of("string.quoted.double"),
        comment: color_of("comment.line"),
        function: color_of("entity.name.function"),
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ThemeError {
    #[error("unknown theme {name:?}; available: {available}")]
    Unknown { name: String, available: String },
    #[error("failed to read theme {}: {source}", path.display())]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to parse theme {}: {source}", path.display())]
    Parse {
        path: PathBuf,
        #[source]
        source: toml::de::Error,
    },
    #[error("failed to load syntax theme {name:?}: {source}")]
    Syntax {
        name: String,
        #[source]
        source: syntect::LoadingError,
    },
}

const BUILTIN_CHROME: &[(&str, &str)] = &[
    ("warm", include_str!("../assets/themes/warm.toml")),
    ("midnight", include_str!("../assets/themes/midnight.toml")),
    ("paper", include_str!("../assets/themes/paper.toml")),
    ("dracula", include_str!("../assets/themes/dracula.toml")),
    ("nord", include_str!("../assets/themes/nord.toml")),
    ("gruvbox", include_str!("../assets/themes/gruvbox.toml")),
    (
        "solarized-dark",
        include_str!("../assets/themes/solarized-dark.toml"),
    ),
    (
        "solarized-light",
        include_str!("../assets/themes/solarized-light.toml"),
    ),
    (
        "catppuccin-mocha",
        include_str!("../assets/themes/catppuccin-mocha.toml"),
    ),
    (
        "catppuccin-latte",
        include_str!("../assets/themes/catppuccin-latte.toml"),
    ),
    ("one-dark", include_str!("../assets/themes/one-dark.toml")),
    (
        "github-light",
        include_str!("../assets/themes/github-light.toml"),
    ),
    (
        "tokyo-night",
        include_str!("../assets/themes/tokyo-night.toml"),
    ),
    ("rose-pine", include_str!("../assets/themes/rose-pine.toml")),
    (
        "everforest",
        include_str!("../assets/themes/everforest.toml"),
    ),
    ("kanagawa", include_str!("../assets/themes/kanagawa.toml")),
];

const BUILTIN_SYNTAX: &[(&str, &[u8])] = &[
    ("warm", include_bytes!("../assets/themes/warm.tmTheme")),
    (
        "tokyo-night",
        include_bytes!("../assets/themes/tokyo-night.tmTheme"),
    ),
    (
        "rose-pine",
        include_bytes!("../assets/themes/rose-pine.tmTheme"),
    ),
    (
        "everforest",
        include_bytes!("../assets/themes/everforest.tmTheme"),
    ),
    (
        "kanagawa",
        include_bytes!("../assets/themes/kanagawa.tmTheme"),
    ),
];

pub struct ThemeRegistry {
    chrome: BTreeMap<String, ChromeTheme>,
    syntax: syntect::highlighting::ThemeSet,
}

impl ThemeRegistry {
    pub fn new() -> Self {
        let mut chrome = BTreeMap::new();
        for (name, text) in BUILTIN_CHROME {
            let theme: ChromeTheme = toml::from_str(text)
                .unwrap_or_else(|e| panic!("built-in chrome theme {name:?} is invalid: {e}"));
            chrome.insert((*name).to_string(), theme);
        }

        let mut syntax = syntect::highlighting::ThemeSet::from(&two_face::theme::extra());
        for (name, bytes) in BUILTIN_SYNTAX {
            let theme = syntect::highlighting::ThemeSet::load_from_reader(
                &mut std::io::Cursor::new(*bytes),
            )
            .unwrap_or_else(|e| panic!("built-in {name}.tmTheme is invalid: {e}"));
            syntax.themes.insert((*name).to_string(), theme);
        }

        Self { chrome, syntax }
    }

    pub fn load_dir(&mut self, dir: &Path) -> Result<(), ThemeError> {
        let entries = match std::fs::read_dir(dir) {
            Ok(e) => e,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(source) => {
                return Err(ThemeError::Io {
                    path: dir.to_path_buf(),
                    source,
                })
            }
        };

        for entry in entries.flatten() {
            let path = entry.path();
            let Some(stem) = path
                .file_stem()
                .and_then(|s| s.to_str())
                .map(str::to_string)
            else {
                continue;
            };
            match path.extension().and_then(|e| e.to_str()) {
                Some("toml") => {
                    let text = std::fs::read_to_string(&path).map_err(|source| ThemeError::Io {
                        path: path.clone(),
                        source,
                    })?;
                    let theme: ChromeTheme =
                        toml::from_str(&text).map_err(|source| ThemeError::Parse {
                            path: path.clone(),
                            source,
                        })?;
                    self.chrome.insert(stem, theme);
                }
                Some("tmTheme") => {
                    let theme =
                        syntect::highlighting::ThemeSet::get_theme(&path).map_err(|source| {
                            ThemeError::Syntax {
                                name: stem.clone(),
                                source,
                            }
                        })?;
                    self.syntax.themes.insert(stem, theme);
                }
                _ => {}
            }
        }
        Ok(())
    }

    pub fn chrome(&self, name: &str) -> Result<&ChromeTheme, ThemeError> {
        self.chrome.get(name).ok_or_else(|| ThemeError::Unknown {
            name: name.to_string(),
            available: self.chrome_names().join(", "),
        })
    }

    pub fn syntax(&self, name: &str) -> Result<&syntect::highlighting::Theme, ThemeError> {
        self.syntax
            .themes
            .get(name)
            .ok_or_else(|| ThemeError::Unknown {
                name: name.to_string(),
                available: self.syntax_names().join(", "),
            })
    }

    pub fn chrome_names(&self) -> Vec<&str> {
        self.chrome.keys().map(String::as_str).collect()
    }

    pub fn syntax_names(&self) -> Vec<&str> {
        self.syntax.themes.keys().map(String::as_str).collect()
    }
}

impl Default for ThemeRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_builtin_chrome_theme_parses() {
        let reg = ThemeRegistry::new();
        for (name, _) in BUILTIN_CHROME {
            let theme = reg.chrome(name).unwrap();
            assert_eq!(&theme.name, name, "theme {name:?} has a mismatched name");
            reg.syntax(&theme.syntax_theme)
                .unwrap_or_else(|e| panic!("theme {name:?} names a missing syntax theme: {e}"));
        }
    }

    #[test]
    fn warm_theme_preserves_the_original_palette() {
        let reg = ThemeRegistry::new();
        let warm = reg.chrome("warm").unwrap();
        assert_eq!(warm.window, Some(Rgba::rgb(0x1a, 0x1a, 0x1a)));
        assert_eq!(warm.titlebar, Some(Rgba::rgb(0x2d, 0x2d, 0x2d)));
        assert_eq!(
            warm.traffic_lights,
            [
                Rgba::rgb(0xC1, 0x5F, 0x3C),
                Rgba::rgb(0xD4, 0x76, 0x3F),
                Rgba::rgb(0xE8, 0x91, 0x42),
            ]
        );
        assert_eq!(
            warm.background,
            Background::Solid {
                color: Rgba::rgb(0xC1, 0x5F, 0x3C)
            }
        );
    }

    #[test]
    fn warm_syntax_theme_is_registered_with_the_right_background() {
        let reg = ThemeRegistry::new();
        let theme = reg.syntax("warm").unwrap();
        let bg = theme.settings.background.unwrap();
        assert_eq!((bg.r, bg.g, bg.b), (0x1a, 0x1a, 0x1a));
    }

    #[test]
    fn syntect_defaults_are_available_too() {
        let reg = ThemeRegistry::new();
        reg.syntax("InspiredGitHub").unwrap();
        reg.syntax("base16-ocean.dark").unwrap();
    }

    #[test]
    fn unknown_theme_lists_the_alternatives() {
        let reg = ThemeRegistry::new();
        let err = reg.chrome("nope").unwrap_err().to_string();
        assert!(err.contains("nope"), "{err}");
        assert!(err.contains("warm"), "error should list options: {err}");
    }

    #[test]
    fn titlebar_is_derived_when_unset() {
        let dark = ChromeTheme {
            window: Some(Rgba::rgb(0x10, 0x10, 0x10)),
            titlebar: None,
            ..Default::default()
        };
        assert!(
            dark.titlebar_color(Rgba::BLACK).luminance()
                > dark.window_color(Rgba::BLACK).luminance()
        );

        let light = ChromeTheme {
            window: Some(Rgba::WHITE),
            titlebar: None,
            ..Default::default()
        };
        assert!(
            light.titlebar_color(Rgba::WHITE).luminance()
                < light.window_color(Rgba::WHITE).luminance()
        );
    }

    #[test]
    fn window_color_falls_back_to_the_syntax_background() {
        let theme = ChromeTheme {
            window: None,
            ..Default::default()
        };
        assert_eq!(theme.window_color(Rgba::rgb(1, 2, 3)), Rgba::rgb(1, 2, 3));
    }

    #[test]
    fn accents_come_from_the_themes_own_rules() {
        let reg = ThemeRegistry::new();
        let warm = accents_of(reg.syntax("warm").unwrap());

        assert_eq!(warm.background, Rgba::rgb(0x1a, 0x1a, 0x1a));
        assert_eq!(warm.foreground, Rgba::rgb(0xF4, 0xF3, 0xEE));
        assert_eq!(warm.keyword, Rgba::rgb(0xE8, 0x91, 0x42));
        assert_eq!(warm.string, Rgba::rgb(0x87, 0xC3, 0x8F));
        assert_eq!(warm.comment, Rgba::rgb(0xB1, 0xAD, 0xA1));
        assert_eq!(warm.function, Rgba::rgb(0xD4, 0x76, 0x3F));
    }

    #[test]
    fn every_syntax_theme_yields_usable_accents() {
        let reg = ThemeRegistry::new();
        for name in reg.syntax_names() {
            let accents = accents_of(reg.syntax(name).unwrap());
            for (label, color) in [
                ("foreground", accents.foreground),
                ("keyword", accents.keyword),
                ("comment", accents.comment),
                ("string", accents.string),
            ] {
                assert_eq!(color.a, 255, "{name}: {label} is translucent");
                assert_ne!(
                    color, accents.background,
                    "{name}: {label} matches the background"
                );
            }
        }
    }

    #[test]
    fn terminal_themes_resolve_to_visible_colors() {
        let ansi_red = syntect::highlighting::Color {
            r: 1,
            g: 0,
            b: 0,
            a: 0,
        };
        assert_eq!(resolve_color(ansi_red, Rgba::WHITE), ANSI_PALETTE[1]);
        let bogus = syntect::highlighting::Color {
            r: 200,
            g: 0,
            b: 0,
            a: 0,
        };
        assert_eq!(resolve_color(bogus, Rgba::WHITE), ANSI_PALETTE[15]);
        let default = syntect::highlighting::Color {
            r: 0,
            g: 0,
            b: 0,
            a: 1,
        };
        assert_eq!(
            resolve_color(default, Rgba::rgb(9, 9, 9)),
            Rgba::rgb(9, 9, 9)
        );
        let plain = syntect::highlighting::Color {
            r: 10,
            g: 20,
            b: 30,
            a: 128,
        };
        assert_eq!(
            resolve_color(plain, Rgba::WHITE),
            Rgba::new(10, 20, 30, 128)
        );
    }

    #[test]
    fn a_light_and_a_dark_theme_differ_in_the_expected_direction() {
        let reg = ThemeRegistry::new();
        let dark = accents_of(reg.syntax("warm").unwrap());
        let light = accents_of(reg.syntax("InspiredGitHub").unwrap());
        assert!(dark.background.is_dark());
        assert!(!light.background.is_dark());
    }

    #[test]
    fn missing_theme_dir_is_not_an_error() {
        let mut reg = ThemeRegistry::new();
        reg.load_dir(Path::new("/nonexistent/snapcode/themes"))
            .unwrap();
    }
}
