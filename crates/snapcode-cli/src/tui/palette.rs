// SPDX-License-Identifier: MIT
//! The TUI's colors, derived from whichever theme is currently selected.

use ratatui::style::{Color, Modifier, Style};
use snapcode_core::color::Rgba;
use snapcode_core::theme::{ChromeTheme, SyntaxAccents};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Palette {
    pub background: Rgba,
    pub surface: Rgba,
    pub foreground: Rgba,
    pub muted: Rgba,
    pub accent: Rgba,
    pub positive: Rgba,
    pub on_accent: Rgba,
}

impl Palette {
    pub fn new(chrome: &ChromeTheme, accents: &SyntaxAccents) -> Self {
        let background = chrome.window_color(accents.background).opaque();
        let surface = chrome.titlebar_color(accents.background).over(background);

        let foreground = readable(accents.foreground, background, FOREGROUND_CONTRAST);
        let muted = readable(accents.comment, background, MUTED_CONTRAST);
        let accent = readable(accents.keyword, background, ACCENT_CONTRAST);
        let positive = readable(accents.string, background, ACCENT_CONTRAST);

        Self {
            background,
            surface,
            foreground,
            muted,
            accent,
            positive,
            on_accent: contrasting(accent),
        }
    }

    pub fn fg(&self, color: Rgba) -> Style {
        Style::default()
            .fg(to_color(color))
            .bg(to_color(self.background))
    }

    pub fn selection(&self) -> Style {
        Style::default()
            .fg(to_color(self.on_accent))
            .bg(to_color(self.accent))
            .add_modifier(Modifier::BOLD)
    }

    pub fn bar(&self) -> Style {
        Style::default()
            .fg(to_color(self.muted))
            .bg(to_color(self.surface))
    }

    pub fn base(&self) -> Style {
        self.fg(self.foreground)
    }

    pub fn label(&self) -> Style {
        self.fg(self.muted)
    }

    pub fn border(&self) -> Style {
        self.fg(self.muted)
    }

    pub fn title(&self) -> Style {
        self.fg(self.accent).add_modifier(Modifier::BOLD)
    }
}

const FOREGROUND_CONTRAST: f32 = 7.0;
const MUTED_CONTRAST: f32 = 4.0;
const ACCENT_CONTRAST: f32 = 4.5;

fn contrast_ratio(a: Rgba, b: Rgba) -> f32 {
    let (l1, l2) = (a.luminance(), b.luminance());
    let (lighter, darker) = if l1 >= l2 { (l1, l2) } else { (l2, l1) };
    (lighter + 0.05) / (darker + 0.05)
}

fn contrasting(background: Rgba) -> Rgba {
    if contrast_ratio(Rgba::WHITE, background) >= contrast_ratio(Rgba::BLACK, background) {
        Rgba::WHITE
    } else {
        Rgba::BLACK
    }
}

fn readable(color: Rgba, background: Rgba, min_ratio: f32) -> Rgba {
    let mut out = color.over(background);
    let target = contrasting(background);
    for _ in 0..40 {
        if contrast_ratio(out, background) >= min_ratio {
            break;
        }
        out = out.mix_rgb(target, 0.08);
    }
    out
}

pub fn to_color(c: Rgba) -> Color {
    Color::Rgb(c.r, c.g, c.b)
}

#[cfg(test)]
mod tests {
    use super::*;
    use snapcode_core::theme::{accents_of, ThemeRegistry};

    fn palette_for(chrome_name: &str) -> Palette {
        let registry = ThemeRegistry::new();
        let chrome = registry.chrome(chrome_name).unwrap();
        let accents = accents_of(registry.syntax(&chrome.syntax_theme).unwrap());
        Palette::new(chrome, &accents)
    }

    fn assert_legible(palette: &Palette, what: &str) {
        for (label, color, required) in [
            ("foreground", palette.foreground, FOREGROUND_CONTRAST),
            ("muted", palette.muted, MUTED_CONTRAST),
            ("accent", palette.accent, ACCENT_CONTRAST),
            ("positive", palette.positive, ACCENT_CONTRAST),
        ] {
            let ratio = contrast_ratio(color, palette.background);
            assert!(
                ratio >= required - 0.05,
                "{what}: {label} {color:?} on {:?} is {ratio:.2}:1, needs {required}:1",
                palette.background
            );
            assert_eq!(color.a, 255, "{what}: {label} must be opaque");
        }
        let selection = contrast_ratio(palette.on_accent, palette.accent);
        assert!(
            selection >= 4.5,
            "{what}: selected-row text is {selection:.2}:1"
        );
        let bar = contrast_ratio(palette.muted, palette.surface);
        assert!(bar >= 3.0, "{what}: status bar text is {bar:.2}:1");
    }

    #[test]
    fn every_builtin_theme_yields_a_legible_palette() {
        let registry = ThemeRegistry::new();
        for name in registry.chrome_names() {
            assert_legible(&palette_for(name), name);
        }
    }

    #[test]
    fn every_theme_pairing_stays_legible() {
        let registry = ThemeRegistry::new();
        for chrome_name in registry.chrome_names() {
            let chrome = registry.chrome(chrome_name).unwrap();
            for syntax_name in registry.syntax_names() {
                let accents = accents_of(registry.syntax(syntax_name).unwrap());
                let palette = Palette::new(chrome, &accents);
                assert_legible(&palette, &format!("{chrome_name} + {syntax_name}"));
            }
        }
    }

    #[test]
    fn a_theme_that_is_already_legible_is_left_alone() {
        let background = Rgba::rgb(0x1a, 0x1a, 0x1a);
        let already_fine = Rgba::rgb(0xF4, 0xF3, 0xEE);
        assert_eq!(
            readable(already_fine, background, FOREGROUND_CONTRAST),
            already_fine
        );
    }

    #[test]
    fn contrast_ratio_matches_the_wcag_range() {
        assert!((contrast_ratio(Rgba::WHITE, Rgba::BLACK) - 21.0).abs() < 0.01);
        assert!((contrast_ratio(Rgba::WHITE, Rgba::WHITE) - 1.0).abs() < 0.01);
        assert_eq!(
            contrast_ratio(Rgba::WHITE, Rgba::BLACK),
            contrast_ratio(Rgba::BLACK, Rgba::WHITE)
        );
    }

    #[test]
    fn a_light_theme_produces_a_light_palette() {
        let paper = palette_for("paper");
        assert!(!paper.background.is_dark(), "paper should be light");
        assert!(paper.foreground.is_dark(), "its text should be dark");

        let warm = palette_for("warm");
        assert!(warm.background.is_dark(), "warm should be dark");
        assert!(!warm.foreground.is_dark(), "its text should be light");
    }

    #[test]
    fn the_palette_tracks_the_theme() {
        assert_ne!(
            palette_for("warm").accent,
            palette_for("midnight").accent,
            "different themes should give different accents"
        );
    }

    #[test]
    fn readable_pushes_a_washed_out_color_until_it_is_legible() {
        let background = Rgba::rgb(0x1a, 0x1a, 0x1a);
        let washed = Rgba::rgb(0x1c, 0x1c, 0x1c);
        let fixed = readable(washed, background, FOREGROUND_CONTRAST);
        assert!(
            contrast_ratio(fixed, background) >= FOREGROUND_CONTRAST - 0.05,
            "{fixed:?} is still unreadable"
        );

        let white = Rgba::WHITE;
        let pastel = Rgba::rgb(0xFF, 0x79, 0xC6);
        let fixed = readable(pastel, white, ACCENT_CONTRAST);
        assert!(
            contrast_ratio(fixed, white) >= ACCENT_CONTRAST - 0.05,
            "{fixed:?} on white is only {:.2}:1",
            contrast_ratio(fixed, white)
        );
    }

    #[test]
    fn translucent_accents_are_flattened() {
        let background = Rgba::rgb(0, 0, 0);
        let half_white = Rgba::new(255, 255, 255, 128);
        let flat = readable(half_white, background, MUTED_CONTRAST);
        assert_eq!(flat.a, 255);
    }

    #[test]
    fn contrast_flips_with_the_background() {
        assert_eq!(contrasting(Rgba::BLACK), Rgba::WHITE);
        assert_eq!(contrasting(Rgba::WHITE), Rgba::BLACK);
        assert_eq!(contrasting(Rgba::rgb(128, 128, 128)), Rgba::BLACK);
    }
}
