// SPDX-License-Identifier: MIT
//! Text shaping: styled spans in, positioned glyphs out.

use cosmic_text::{Buffer, Color, LayoutGlyph, Metrics, Shaping, Wrap};

use crate::color::Rgba;
use crate::config::{is_emphasized, RenderConfig};
use crate::font::FontStack;
use crate::highlight::{DiffKind, Highlighted, HighlightedLine};

#[derive(Debug, Clone)]
pub struct ShapedGlyph {
    pub inner: LayoutGlyph,
    pub color: Rgba,
}

#[derive(Debug, Clone)]
pub struct ShapedRow {
    pub glyphs: Vec<ShapedGlyph>,
    pub width: f32,
    pub baseline: f32,
}

#[derive(Debug, Clone)]
pub struct ShapedLine {
    pub rows: Vec<ShapedRow>,
    pub number: u32,
    pub diff: Option<DiffKind>,
    pub emphasized: bool,
}

#[derive(Debug, Clone)]
pub struct ShapedText {
    pub lines: Vec<ShapedLine>,
    pub max_width: f32,
    pub row_height: f32,
    pub total_rows: usize,
}

impl ShapedText {
    pub fn height(&self) -> f32 {
        self.total_rows as f32 * self.row_height
    }
}

pub fn shape(
    highlighted: &Highlighted,
    config: &RenderConfig,
    fonts: &mut FontStack,
) -> ShapedText {
    let font_size = config.px(config.font.size);
    let row_height = (font_size * config.font.line_height).max(1.0);
    let metrics = Metrics::new(font_size, row_height);

    let wrap_width = config.code.max_width.map(|cols| {
        let cell = measure_cell_width(fonts, metrics);
        cell * cols as f32
    });

    let mut buffer = Buffer::new(fonts.system_mut(), metrics);
    let wrap = match (config.code.wrap, wrap_width) {
        (true, Some(_)) => Wrap::WordOrGlyph,
        _ => Wrap::None,
    };
    buffer.set_wrap(fonts.system_mut(), wrap);
    buffer.set_size(fonts.system_mut(), wrap_width, None);

    let dim_target = highlighted.background;
    let mut lines = Vec::with_capacity(highlighted.lines.len());
    let mut max_width: f32 = 0.0;
    let mut total_rows = 0usize;

    for line in &highlighted.lines {
        let emphasized = is_emphasized(&config.code.highlight_lines, line.number);
        let rows = shape_line(line, emphasized, config, fonts, &mut buffer, dim_target);
        for row in &rows {
            max_width = max_width.max(row.width);
        }
        total_rows += rows.len();
        lines.push(ShapedLine {
            rows,
            number: line.number,
            diff: line.diff,
            emphasized,
        });
    }

    ShapedText {
        lines,
        max_width,
        row_height,
        total_rows,
    }
}

fn shape_line(
    line: &HighlightedLine,
    emphasized: bool,
    config: &RenderConfig,
    fonts: &mut FontStack,
    buffer: &mut Buffer,
    dim_target: Rgba,
) -> Vec<ShapedRow> {
    let spans: Vec<(String, bool, bool, Rgba)> = clip_spans(line, config);

    if spans.is_empty() {
        return vec![ShapedRow {
            glyphs: Vec::new(),
            width: 0.0,
            baseline: config.px(config.font.size),
        }];
    }

    let dim = if emphasized {
        0.0
    } else {
        config.code.dim_amount
    };
    let (system, styles) = fonts.split();
    let attr_spans: Vec<(&str, cosmic_text::Attrs<'_>)> = spans
        .iter()
        .map(|(text, bold, italic, color)| {
            let color = color.mix_rgb(dim_target, dim);
            let attrs = styles
                .attrs(*bold, *italic)
                .color(Color::rgba(color.r, color.g, color.b, color.a));
            (text.as_str(), attrs)
        })
        .collect();

    let default_attrs = styles.attrs(false, false);
    buffer.set_rich_text(
        system,
        attr_spans.iter().map(|(t, a)| (*t, a.clone())),
        &default_attrs,
        Shaping::Advanced,
        None,
    );

    let mut rows = Vec::new();
    for run in buffer.layout_runs() {
        let glyphs: Vec<ShapedGlyph> = run
            .glyphs
            .iter()
            .map(|g| ShapedGlyph {
                inner: g.clone(),
                color: g
                    .color_opt
                    .map(|c| Rgba::new(c.r(), c.g(), c.b(), c.a()))
                    .unwrap_or(Rgba::WHITE),
            })
            .collect();

        let advance_width = glyphs
            .iter()
            .map(|g| g.inner.x + g.inner.w)
            .fold(0.0_f32, f32::max);

        rows.push(ShapedRow {
            glyphs,
            width: run.line_w.max(advance_width),
            baseline: run.line_y - run.line_top,
        });
    }

    if rows.is_empty() {
        rows.push(ShapedRow {
            glyphs: Vec::new(),
            width: 0.0,
            baseline: config.px(config.font.size),
        });
    }
    rows
}

fn clip_spans(line: &HighlightedLine, config: &RenderConfig) -> Vec<(String, bool, bool, Rgba)> {
    let limit = match (config.code.wrap, config.code.max_width) {
        (false, Some(cols)) => Some(cols),
        _ => None,
    };

    let mut out = Vec::with_capacity(line.spans.len());
    let mut used = 0usize;
    for span in &line.spans {
        if span.text.is_empty() {
            continue;
        }
        let text = match limit {
            None => span.text.clone(),
            Some(limit) => {
                if used >= limit {
                    break;
                }
                let room = limit - used;
                let taken: String = span.text.chars().take(room).collect();
                used += taken.chars().count();
                taken
            }
        };
        if !text.is_empty() {
            out.push((text, span.bold, span.italic, span.color));
        }
    }
    out
}

fn measure_cell_width(fonts: &mut FontStack, metrics: Metrics) -> f32 {
    let (system, styles) = fonts.split();
    let attrs = styles.attrs(false, false);
    let mut buffer = Buffer::new(system, metrics);
    buffer.set_wrap(system, Wrap::None);
    buffer.set_size(system, None, None);
    buffer.set_rich_text(
        system,
        [("0", attrs.clone())],
        &attrs,
        Shaping::Advanced,
        None,
    );
    buffer
        .layout_runs()
        .next()
        .map(|run| run.line_w)
        .filter(|w| *w > 0.0)
        .unwrap_or(metrics.font_size * 0.6)
}

pub fn shape_label(
    text: &str,
    color: Rgba,
    font_size: f32,
    row_height: f32,
    fonts: &mut FontStack,
) -> ShapedRow {
    let metrics = Metrics::new(font_size, row_height.max(1.0));
    let (system, styles) = fonts.split();
    let attrs = styles
        .attrs(false, false)
        .color(Color::rgba(color.r, color.g, color.b, color.a));
    let default_attrs = styles.attrs(false, false);
    let mut buffer = Buffer::new(system, metrics);
    buffer.set_wrap(system, Wrap::None);
    buffer.set_size(system, None, None);
    buffer.set_rich_text(
        system,
        [(text, attrs)],
        &default_attrs,
        Shaping::Advanced,
        None,
    );

    buffer
        .layout_runs()
        .next()
        .map(|run| {
            let glyphs: Vec<ShapedGlyph> = run
                .glyphs
                .iter()
                .map(|g| ShapedGlyph {
                    inner: g.clone(),
                    color,
                })
                .collect();
            let advance = glyphs
                .iter()
                .map(|g| g.inner.x + g.inner.w)
                .fold(0.0_f32, f32::max);
            ShapedRow {
                glyphs,
                width: run.line_w.max(advance),
                baseline: run.line_y - run.line_top,
            }
        })
        .unwrap_or(ShapedRow {
            glyphs: Vec::new(),
            width: 0.0,
            baseline: font_size,
        })
}

#[cfg(test)]
#[allow(clippy::field_reassign_with_default)]
mod tests {
    use super::*;
    use crate::config::CodeConfig;
    use crate::font::FontStack;
    use crate::highlight::Highlighter;
    use crate::theme::ThemeRegistry;

    fn shape_source(source: &str, config: &RenderConfig) -> ShapedText {
        let hl = Highlighter::new();
        let themes = ThemeRegistry::new();
        let syntax = hl.resolve(Some("swift"), None, "").unwrap();
        let theme = themes.syntax("warm").unwrap();
        let highlighted = hl.highlight(source, syntax, theme, &config.code).unwrap();
        let mut fonts = FontStack::embedded_only(&config.font).unwrap();
        shape(&highlighted, config, &mut fonts)
    }

    #[test]
    fn shapes_one_row_per_line() {
        let out = shape_source("let a = 1\nlet b = 2\n", &RenderConfig::default());
        assert_eq!(out.lines.len(), 2);
        assert!(out.lines.iter().all(|l| l.rows.len() == 1));
        assert_eq!(out.total_rows, 2);
    }

    #[test]
    fn row_height_follows_scale_and_line_height() {
        let mut config = RenderConfig::default();
        config.scale = 1.0;
        let out = shape_source("x", &config);
        assert!((out.row_height - 28.0).abs() < 0.01, "{}", out.row_height);

        config.scale = 2.0;
        let out = shape_source("x", &config);
        assert!((out.row_height - 56.0).abs() < 0.01, "{}", out.row_height);
    }

    #[test]
    fn leading_indentation_shifts_glyphs_right() {
        let config = RenderConfig::default();
        let plain = shape_source("x", &config);
        let indented = shape_source("        x", &config);

        let last_x = |t: &ShapedText| t.lines[0].rows[0].glyphs.last().unwrap().inner.x;
        assert!(
            last_x(&indented) > last_x(&plain) + 1.0,
            "indent lost: {} vs {}",
            last_x(&indented),
            last_x(&plain)
        );
        assert_eq!(plain.lines[0].rows[0].glyphs.len(), 1);
        assert_eq!(indented.lines[0].rows[0].glyphs.len(), 9);
        assert!(indented.max_width > plain.max_width);
    }

    #[test]
    fn trailing_whitespace_counts_toward_width() {
        let config = RenderConfig::default();
        let bare = shape_source("x", &config);
        let padded = shape_source("x    ", &config);
        assert!(
            padded.max_width > bare.max_width,
            "trailing space ignored: {} vs {}",
            padded.max_width,
            bare.max_width
        );
    }

    #[test]
    fn max_width_clips_when_wrapping_is_off() {
        let mut config = RenderConfig::default();
        config.code = CodeConfig {
            max_width: Some(5),
            wrap: false,
            ..Default::default()
        };
        let out = shape_source("abcdefghijklmnop", &config);
        assert_eq!(out.lines[0].rows.len(), 1, "clipping must not wrap");
        assert_eq!(out.lines[0].rows[0].glyphs.len(), 5);
    }

    #[test]
    fn max_width_wraps_into_extra_rows_when_enabled() {
        let mut config = RenderConfig::default();
        config.code = CodeConfig {
            max_width: Some(10),
            wrap: true,
            ..Default::default()
        };
        let out = shape_source("aaaa bbbb cccc dddd eeee", &config);
        assert!(
            out.lines[0].rows.len() > 1,
            "expected wrapping, got {} row(s)",
            out.lines[0].rows.len()
        );
        assert_eq!(out.total_rows, out.lines[0].rows.len());
    }

    #[test]
    fn wrapped_rows_are_one_row_height_apart() {
        let mut config = RenderConfig::default();
        config.scale = 1.0;
        config.code = CodeConfig {
            max_width: Some(10),
            wrap: true,
            ..Default::default()
        };
        let out = shape_source("aaaa bbbb cccc dddd eeee", &config);
        let rows = &out.lines[0].rows;
        assert!(rows.len() >= 3, "expected wrapping, got {}", rows.len());

        for row in rows {
            assert!(
                row.baseline >= 0.0 && row.baseline <= out.row_height,
                "baseline {} must be an offset inside its own row (height {})",
                row.baseline,
                out.row_height
            );
        }
        let first = rows[0].baseline;
        assert!(
            rows.iter().all(|r| (r.baseline - first).abs() < 0.01),
            "every row shares the same in-row baseline: {:?}",
            rows.iter().map(|r| r.baseline).collect::<Vec<_>>()
        );
    }

    #[test]
    fn empty_lines_still_occupy_a_row() {
        let out = shape_source("a\n\nb\n", &RenderConfig::default());
        assert_eq!(out.lines.len(), 3);
        assert!(out.lines[1].rows[0].glyphs.is_empty());
        assert_eq!(out.lines[1].rows[0].width, 0.0);
        assert_eq!(out.total_rows, 3);
    }

    #[test]
    fn glyph_colors_come_from_the_syntax_theme() {
        let out = shape_source("// comment", &RenderConfig::default());
        let color = out.lines[0].rows[0].glyphs[0].color;
        assert_eq!(color, Rgba::rgb(0xB1, 0xAD, 0xA1));
    }

    #[test]
    fn unhighlighted_lines_are_dimmed_toward_the_background() {
        let mut config = RenderConfig::default();
        config.code.highlight_lines = crate::config::LineRange::parse_list("1").unwrap();
        let out = shape_source("// one\n// two\n", &config);

        assert!(out.lines[0].emphasized);
        assert!(!out.lines[1].emphasized);

        let bright = out.lines[0].rows[0].glyphs[0].color;
        let dimmed = out.lines[1].rows[0].glyphs[0].color;
        assert_ne!(bright, dimmed);
        assert!(
            dimmed.luminance() < bright.luminance(),
            "{dimmed:?} should be dimmer than {bright:?}"
        );
    }

    #[test]
    fn max_width_is_the_widest_row() {
        let out = shape_source("x\nxxxxxxxxxx\nxx\n", &RenderConfig::default());
        let widest = out
            .lines
            .iter()
            .flat_map(|l| &l.rows)
            .map(|r| r.width)
            .fold(0.0_f32, f32::max);
        assert!((out.max_width - widest).abs() < 0.01);
        assert!((out.max_width - out.lines[1].rows[0].width).abs() < 0.01);
    }

    #[test]
    fn labels_shape_independently() {
        let config = RenderConfig::default();
        let mut fonts = FontStack::embedded_only(&config.font).unwrap();
        let row = shape_label("123", Rgba::WHITE, 20.0, 28.0, &mut fonts);
        assert_eq!(row.glyphs.len(), 3);
        assert!(row.width > 0.0);
        assert!(row.glyphs.iter().all(|g| g.color == Rgba::WHITE));
    }

    #[test]
    fn height_is_rows_times_row_height() {
        let out = shape_source("a\nb\nc\n", &RenderConfig::default());
        assert_eq!(out.total_rows, 3);
        assert!((out.height() - 3.0 * out.row_height).abs() < 0.01);
    }
}
