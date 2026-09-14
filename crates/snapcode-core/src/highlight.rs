// SPDX-License-Identifier: MIT
//! Syntax highlighting: source text in, styled spans out.

use std::path::Path;

use syntect::easy::HighlightLines;
use syntect::highlighting::{FontStyle, Theme};
use syntect::parsing::{SyntaxReference, SyntaxSet};
use syntect::util::LinesWithEndings;

use crate::color::Rgba;
use crate::config::CodeConfig;

#[derive(Debug, Clone, PartialEq)]
pub struct StyledSpan {
    pub text: String,
    pub color: Rgba,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiffKind {
    Added,
    Removed,
}

#[derive(Debug, Clone, PartialEq)]
pub struct HighlightedLine {
    pub spans: Vec<StyledSpan>,
    pub number: u32,
    pub diff: Option<DiffKind>,
}

impl HighlightedLine {
    pub fn char_len(&self) -> usize {
        self.spans.iter().map(|s| s.text.chars().count()).sum()
    }

    pub fn text(&self) -> String {
        self.spans.iter().map(|s| s.text.as_str()).collect()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Highlighted {
    pub lines: Vec<HighlightedLine>,
    pub background: Rgba,
    pub foreground: Rgba,
    pub language: String,
}

#[derive(Debug, thiserror::Error)]
pub enum HighlightError {
    #[error("unknown language {0:?}")]
    UnknownLanguage(String),
    #[error("highlighting failed: {0}")]
    Syntect(#[from] syntect::Error),
}

pub struct Highlighter {
    syntaxes: SyntaxSet,
}

impl Highlighter {
    pub fn new() -> Self {
        Self {
            syntaxes: two_face::syntax::extra_newlines(),
        }
    }

    pub fn syntaxes(&self) -> &SyntaxSet {
        &self.syntaxes
    }

    pub fn language_names(&self) -> Vec<&str> {
        let mut names: Vec<&str> = self
            .syntaxes
            .syntaxes()
            .iter()
            .map(|s| s.name.as_str())
            .collect();
        names.sort_unstable();
        names.dedup();
        names
    }

    pub fn resolve<'a>(
        &'a self,
        hint: Option<&str>,
        path: Option<&Path>,
        source: &str,
    ) -> Result<&'a SyntaxReference, HighlightError> {
        if let Some(hint) = hint.map(str::trim).filter(|h| !h.is_empty()) {
            return self
                .find_by_hint(hint)
                .ok_or_else(|| HighlightError::UnknownLanguage(hint.to_string()));
        }

        if let Some(path) = path {
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                if let Some(s) = self.syntaxes.find_syntax_by_extension(name) {
                    return Ok(s);
                }
            }
            if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                if let Some(s) = self.syntaxes.find_syntax_by_extension(ext) {
                    return Ok(s);
                }
            }
        }

        if let Some(s) = self.syntaxes.find_syntax_by_first_line(source) {
            return Ok(s);
        }

        Ok(self.syntaxes.find_syntax_plain_text())
    }

    fn find_by_hint(&self, hint: &str) -> Option<&SyntaxReference> {
        self.syntaxes
            .find_syntax_by_name(hint)
            .or_else(|| self.syntaxes.find_syntax_by_extension(hint))
            .or_else(|| self.syntaxes.find_syntax_by_token(hint))
            .or_else(|| {
                let lower = hint.to_ascii_lowercase();
                self.syntaxes.syntaxes().iter().find(|s| {
                    s.name.to_ascii_lowercase() == lower
                        || s.file_extensions
                            .iter()
                            .any(|e| e.to_ascii_lowercase() == lower)
                })
            })
    }

    pub fn highlight(
        &self,
        source: &str,
        syntax: &SyntaxReference,
        theme: &Theme,
        code: &CodeConfig,
    ) -> Result<Highlighted, HighlightError> {
        let accents = crate::theme::accents_of(theme);
        let (background, foreground) = (accents.background, accents.foreground);

        let mut highlighter = HighlightLines::new(syntax, theme);
        let mut lines = Vec::new();

        for (index, raw) in LinesWithEndings::from(source).enumerate() {
            let regions = highlighter.highlight_line(raw, &self.syntaxes)?;
            let mut spans = Vec::with_capacity(regions.len());
            let mut column = 0usize;

            for (style, text) in regions {
                let text = text.trim_end_matches(['\n', '\r']);
                if text.is_empty() {
                    continue;
                }
                let expanded = expand_tabs(text, code.tab_width, &mut column);
                spans.push(StyledSpan {
                    text: expanded,
                    color: crate::theme::resolve_color(style.foreground, foreground),
                    bold: style.font_style.contains(FontStyle::BOLD),
                    italic: style.font_style.contains(FontStyle::ITALIC),
                    underline: style.font_style.contains(FontStyle::UNDERLINE),
                });
            }

            let diff = code.diff.then(|| detect_diff(&spans)).flatten();
            lines.push(HighlightedLine {
                spans,
                number: index as u32 + 1,
                diff,
            });
        }

        if lines.is_empty() {
            lines.push(HighlightedLine {
                spans: Vec::new(),
                number: 1,
                diff: None,
            });
        }

        Ok(Highlighted {
            lines,
            background,
            foreground,
            language: syntax.name.clone(),
        })
    }
}

impl Default for Highlighter {
    fn default() -> Self {
        Self::new()
    }
}

fn expand_tabs(text: &str, tab_width: usize, column: &mut usize) -> String {
    if !text.contains('\t') {
        *column += text.chars().count();
        return text.to_string();
    }
    let width = tab_width.max(1);
    let mut out = String::with_capacity(text.len() + width);
    for ch in text.chars() {
        if ch == '\t' {
            let stop = width - (*column % width);
            out.extend(std::iter::repeat_n(' ', stop));
            *column += stop;
        } else {
            out.push(ch);
            *column += 1;
        }
    }
    out
}

fn detect_diff(spans: &[StyledSpan]) -> Option<DiffKind> {
    let first = spans.iter().find_map(|s| s.text.chars().next())?;
    match first {
        '+' => Some(DiffKind::Added),
        '-' => Some(DiffKind::Removed),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::ThemeRegistry;

    fn setup() -> (Highlighter, ThemeRegistry) {
        (Highlighter::new(), ThemeRegistry::new())
    }

    #[test]
    fn detects_language_from_extension_and_hint() {
        let (hl, _) = setup();
        let by_ext = hl.resolve(None, Some(Path::new("a.swift")), "").unwrap();
        assert_eq!(by_ext.name, "Swift");

        for hint in ["Rust", "rust", "rs"] {
            assert_eq!(hl.resolve(Some(hint), None, "").unwrap().name, "Rust");
        }
    }

    #[test]
    fn falls_back_to_plain_text_but_reports_a_bad_hint() {
        let (hl, _) = setup();
        let s = hl.resolve(None, Some(Path::new("a.zzz")), "hello").unwrap();
        assert_eq!(s.name, "Plain Text");
        assert!(matches!(
            hl.resolve(Some("swfit"), None, ""),
            Err(HighlightError::UnknownLanguage(_))
        ));
    }

    #[test]
    fn detects_language_from_a_shebang() {
        let (hl, _) = setup();
        let s = hl
            .resolve(None, None, "#!/usr/bin/env python3\nx = 1\n")
            .unwrap();
        assert_eq!(s.name, "Python");
    }

    #[test]
    fn highlights_swift_with_distinct_colors() {
        let (hl, themes) = setup();
        let syntax = hl.resolve(Some("swift"), None, "").unwrap();
        let theme = themes.syntax("warm").unwrap();
        let out = hl
            .highlight("// hi\nlet x = 42\n", syntax, theme, &CodeConfig::default())
            .unwrap();

        assert_eq!(out.lines.len(), 2);
        assert_eq!(out.language, "Swift");
        assert_eq!(out.background, Rgba::rgb(0x1a, 0x1a, 0x1a));
        assert_eq!(out.lines[0].text(), "// hi");
        assert_eq!(out.lines[1].text(), "let x = 42");
        assert_eq!(out.lines[1].number, 2);

        assert_eq!(out.lines[0].spans[0].color, Rgba::rgb(0xB1, 0xAD, 0xA1));
        let colors: Vec<Rgba> = out.lines[1].spans.iter().map(|s| s.color).collect();
        assert!(colors.contains(&Rgba::rgb(0xE8, 0x91, 0x42)), "{colors:?}");
        assert!(colors.contains(&Rgba::rgb(0x9D, 0xB4, 0xC0)), "{colors:?}");
    }

    #[test]
    fn strips_newlines_from_spans() {
        let (hl, themes) = setup();
        let syntax = hl.resolve(Some("swift"), None, "").unwrap();
        let out = hl
            .highlight(
                "let a = 1\nlet b = 2\n",
                syntax,
                themes.syntax("warm").unwrap(),
                &CodeConfig::default(),
            )
            .unwrap();
        for line in &out.lines {
            for span in &line.spans {
                assert!(
                    !span.text.contains('\n') && !span.text.contains('\r'),
                    "span leaked a newline: {:?}",
                    span.text
                );
            }
        }
    }

    #[test]
    fn preserves_indentation_including_trailing_space() {
        let (hl, themes) = setup();
        let syntax = hl.resolve(Some("swift"), None, "").unwrap();
        let out = hl
            .highlight(
                "    let x = 1  ",
                syntax,
                themes.syntax("warm").unwrap(),
                &CodeConfig::default(),
            )
            .unwrap();
        assert!(
            out.lines[0].text().starts_with("    "),
            "{:?}",
            out.lines[0].text()
        );
        assert!(
            out.lines[0].text().ends_with("  "),
            "{:?}",
            out.lines[0].text()
        );
    }

    #[test]
    fn expands_tabs_to_tab_stops() {
        let mut col = 0;
        assert_eq!(expand_tabs("\tx", 4, &mut col), "    x");
        let mut col = 4;
        assert_eq!(expand_tabs("\t", 4, &mut col), "    ");
        let mut col = 2;
        assert_eq!(expand_tabs("\t", 4, &mut col), "  ");
        let mut col = 0;
        assert_eq!(expand_tabs("ab", 4, &mut col), "ab");
        assert_eq!(expand_tabs("\t", 4, &mut col), "  ");
    }

    #[test]
    fn empty_input_still_yields_one_line() {
        let (hl, themes) = setup();
        let syntax = hl.resolve(Some("swift"), None, "").unwrap();
        let out = hl
            .highlight(
                "",
                syntax,
                themes.syntax("warm").unwrap(),
                &CodeConfig::default(),
            )
            .unwrap();
        assert_eq!(out.lines.len(), 1);
        assert_eq!(out.lines[0].char_len(), 0);
    }

    #[test]
    fn diff_lines_are_classified_only_when_enabled() {
        let (hl, themes) = setup();
        let syntax = hl.resolve(Some("swift"), None, "").unwrap();
        let theme = themes.syntax("warm").unwrap();
        let src = "+added\n-removed\n unchanged\n";

        let off = hl
            .highlight(src, syntax, theme, &CodeConfig::default())
            .unwrap();
        assert!(off.lines.iter().all(|l| l.diff.is_none()));

        let code = CodeConfig {
            diff: true,
            ..Default::default()
        };
        let on = hl.highlight(src, syntax, theme, &code).unwrap();
        assert_eq!(on.lines[0].diff, Some(DiffKind::Added));
        assert_eq!(on.lines[1].diff, Some(DiffKind::Removed));
        assert_eq!(on.lines[2].diff, None);
    }

    #[test]
    fn language_list_is_sorted_and_populated() {
        let (hl, _) = setup();
        let names = hl.language_names();
        assert!(
            names.len() > 50,
            "expected many syntaxes, got {}",
            names.len()
        );
        assert!(names.contains(&"Swift"));
        assert!(names.windows(2).all(|w| w[0] <= w[1]), "not sorted");
    }
}
