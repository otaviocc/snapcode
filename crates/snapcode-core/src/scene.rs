// SPDX-License-Identifier: MIT
//! A backend-agnostic list of drawing primitives.

use std::sync::Arc;

use crate::color::Rgba;
use crate::config::GradientStop;
use crate::layout::ShapedGlyph;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl Rect {
    pub fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            x,
            y,
            width: width.max(0.0),
            height: height.max(0.0),
        }
    }

    pub fn right(&self) -> f32 {
        self.x + self.width
    }

    pub fn bottom(&self) -> f32 {
        self.y + self.height
    }

    pub fn is_empty(&self) -> bool {
        self.width <= 0.0 || self.height <= 0.0
    }

    pub fn inflate(&self, amount: f32) -> Rect {
        Rect::new(
            self.x - amount,
            self.y - amount,
            self.width + amount * 2.0,
            self.height + amount * 2.0,
        )
    }

    pub fn translate(&self, dx: f32, dy: f32) -> Rect {
        Rect {
            x: self.x + dx,
            y: self.y + dy,
            ..*self
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Corners {
    pub top_left: f32,
    pub top_right: f32,
    pub bottom_right: f32,
    pub bottom_left: f32,
}

impl Corners {
    pub const SQUARE: Corners = Corners::uniform(0.0);

    pub const fn uniform(r: f32) -> Self {
        Self {
            top_left: r,
            top_right: r,
            bottom_right: r,
            bottom_left: r,
        }
    }

    pub const fn top(r: f32) -> Self {
        Self {
            top_left: r,
            top_right: r,
            bottom_right: 0.0,
            bottom_left: 0.0,
        }
    }

    pub const fn bottom(r: f32) -> Self {
        Self {
            top_left: 0.0,
            top_right: 0.0,
            bottom_right: r,
            bottom_left: r,
        }
    }

    pub fn is_square(&self) -> bool {
        self.top_left <= 0.0
            && self.top_right <= 0.0
            && self.bottom_right <= 0.0
            && self.bottom_left <= 0.0
    }

    pub fn clamped(&self, rect: &Rect) -> Corners {
        let limit = (rect.width.min(rect.height) / 2.0).max(0.0);
        Corners {
            top_left: self.top_left.clamp(0.0, limit),
            top_right: self.top_right.clamp(0.0, limit),
            bottom_right: self.bottom_right.clamp(0.0, limit),
            bottom_left: self.bottom_left.clamp(0.0, limit),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ImageData {
    pub width: u32,
    pub height: u32,
    pub pixels: Arc<Vec<u8>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImagePlacement {
    Fill,
    Fit,
    Tile,
    Center,
    Stretch,
}

#[derive(Debug, Clone)]
pub enum Paint {
    Solid(Rgba),
    Linear {
        stops: Vec<GradientStop>,
        angle: f32,
    },
    Radial {
        stops: Vec<GradientStop>,
    },
    Image {
        image: ImageData,
        placement: ImagePlacement,
        blur: f32,
        darken: f32,
    },
}

impl Paint {
    pub fn is_invisible(&self) -> bool {
        match self {
            Paint::Solid(c) => c.is_transparent(),
            Paint::Linear { stops, .. } | Paint::Radial { stops } => {
                stops.is_empty() || stops.iter().all(|s| s.color.is_transparent())
            }
            Paint::Image { .. } => false,
        }
    }
}

#[derive(Debug, Clone)]
pub struct PlacedGlyph {
    pub shaped: ShapedGlyph,
    pub origin_x: f32,
    pub origin_y: f32,
}

#[derive(Debug, Clone)]
pub enum Op {
    Fill {
        rect: Rect,
        corners: Corners,
        paint: Paint,
    },
    Shadow {
        rect: Rect,
        corners: Corners,
        blur: f32,
        color: Rgba,
    },
    Stroke {
        rect: Rect,
        corners: Corners,
        color: Rgba,
        width: f32,
    },
    Ellipse {
        rect: Rect,
        color: Rgba,
    },
    Glyphs(Vec<PlacedGlyph>),
}

#[derive(Debug, Clone)]
pub struct Scene {
    pub width: u32,
    pub height: u32,
    pub ops: Vec<Op>,
}

impl Scene {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width: width.max(1),
            height: height.max(1),
            ops: Vec::new(),
        }
    }

    pub fn push(&mut self, op: Op) {
        let skip = match &op {
            Op::Fill { rect, paint, .. } => rect.is_empty() || paint.is_invisible(),
            Op::Shadow { rect, color, .. } => rect.is_empty() || color.is_transparent(),
            Op::Stroke {
                rect, color, width, ..
            } => rect.is_empty() || color.is_transparent() || *width <= 0.0,
            Op::Ellipse { rect, color } => rect.is_empty() || color.is_transparent(),
            Op::Glyphs(glyphs) => glyphs.is_empty(),
        };
        if !skip {
            self.ops.push(op);
        }
    }

    pub fn bounds(&self) -> Rect {
        Rect::new(0.0, 0.0, self.width as f32, self.height as f32)
    }
}

pub fn normalize_stops(stops: &[GradientStop]) -> Vec<GradientStop> {
    if stops.is_empty() {
        return vec![GradientStop::new(0.0, Rgba::TRANSPARENT)];
    }
    let mut out: Vec<GradientStop> = stops
        .iter()
        .map(|s| GradientStop::new(s.offset.clamp(0.0, 1.0), s.color))
        .collect();
    out.sort_by(|a, b| {
        a.offset
            .partial_cmp(&b.offset)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    out
}

pub fn sample_stops(stops: &[GradientStop], t: f32) -> Rgba {
    debug_assert!(!stops.is_empty(), "stops must be normalized first");
    let t = t.clamp(0.0, 1.0);
    if stops.len() == 1 {
        return stops[0].color;
    }
    if t <= stops[0].offset {
        return stops[0].color;
    }
    let last = stops[stops.len() - 1];
    if t >= last.offset {
        return last.color;
    }
    for pair in stops.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        if t >= a.offset && t <= b.offset {
            let span = b.offset - a.offset;
            let local = if span <= f32::EPSILON {
                1.0
            } else {
                (t - a.offset) / span
            };
            return a.color.lerp(b.color, local);
        }
    }
    last.color
}

pub fn linear_gradient_t(rect: &Rect, angle_degrees: f32, x: f32, y: f32) -> f32 {
    let (sin, cos) = angle_degrees.to_radians().sin_cos();
    let cx = rect.x + rect.width / 2.0;
    let cy = rect.y + rect.height / 2.0;
    let projected = (x - cx) * cos + (y - cy) * sin;
    let extent = (rect.width * cos.abs() + rect.height * sin.abs()) / 2.0;
    if extent <= f32::EPSILON {
        return 0.5;
    }
    (projected / extent + 1.0) / 2.0
}

pub fn radial_gradient_t(rect: &Rect, x: f32, y: f32) -> f32 {
    let cx = rect.x + rect.width / 2.0;
    let cy = rect.y + rect.height / 2.0;
    let radius = (rect.width.powi(2) + rect.height.powi(2)).sqrt() / 2.0;
    if radius <= f32::EPSILON {
        return 0.0;
    }
    (((x - cx).powi(2) + (y - cy).powi(2)).sqrt() / radius).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stops(pairs: &[(f32, Rgba)]) -> Vec<GradientStop> {
        pairs
            .iter()
            .map(|(o, c)| GradientStop::new(*o, *c))
            .collect()
    }

    #[test]
    fn rect_geometry_is_consistent() {
        let r = Rect::new(10.0, 20.0, 100.0, 50.0);
        assert_eq!(r.right(), 110.0);
        assert_eq!(r.bottom(), 70.0);
        assert!(!r.is_empty());
        assert_eq!(r.inflate(5.0), Rect::new(5.0, 15.0, 110.0, 60.0));
        assert_eq!(r.translate(1.0, 2.0), Rect::new(11.0, 22.0, 100.0, 50.0));
        assert!(Rect::new(0.0, 0.0, -5.0, 10.0).is_empty());
    }

    #[test]
    fn corners_clamp_to_half_the_shorter_side() {
        let rect = Rect::new(0.0, 0.0, 40.0, 20.0);
        let clamped = Corners::uniform(1000.0).clamped(&rect);
        assert_eq!(clamped.top_left, 10.0);
        assert_eq!(clamped.bottom_right, 10.0);
        assert_eq!(Corners::uniform(4.0).clamped(&rect).top_left, 4.0);
    }

    #[test]
    fn titlebar_corners_round_only_the_top() {
        let c = Corners::top(10.0);
        assert_eq!((c.top_left, c.top_right), (10.0, 10.0));
        assert_eq!((c.bottom_left, c.bottom_right), (0.0, 0.0));
        assert!(!c.is_square());
        assert!(Corners::SQUARE.is_square());
    }

    #[test]
    fn scene_drops_invisible_ops() {
        let mut scene = Scene::new(100, 100);
        scene.push(Op::Fill {
            rect: Rect::new(0.0, 0.0, 0.0, 10.0),
            corners: Corners::SQUARE,
            paint: Paint::Solid(Rgba::WHITE),
        });
        scene.push(Op::Fill {
            rect: Rect::new(0.0, 0.0, 10.0, 10.0),
            corners: Corners::SQUARE,
            paint: Paint::Solid(Rgba::TRANSPARENT),
        });
        scene.push(Op::Stroke {
            rect: Rect::new(0.0, 0.0, 10.0, 10.0),
            corners: Corners::SQUARE,
            color: Rgba::WHITE,
            width: 0.0,
        });
        scene.push(Op::Glyphs(Vec::new()));
        assert!(scene.ops.is_empty(), "{:?}", scene.ops);

        scene.push(Op::Fill {
            rect: Rect::new(0.0, 0.0, 10.0, 10.0),
            corners: Corners::SQUARE,
            paint: Paint::Solid(Rgba::WHITE),
        });
        assert_eq!(scene.ops.len(), 1);
    }

    #[test]
    fn scene_dimensions_are_never_zero() {
        let scene = Scene::new(0, 0);
        assert_eq!((scene.width, scene.height), (1, 1));
    }

    #[test]
    fn stops_are_sorted_and_clamped() {
        let s = normalize_stops(&stops(&[
            (2.0, Rgba::WHITE),
            (-1.0, Rgba::BLACK),
            (0.5, Rgba::rgb(1, 2, 3)),
        ]));
        assert_eq!(s.len(), 3);
        assert_eq!(s[0].offset, 0.0);
        assert_eq!(s[1].offset, 0.5);
        assert_eq!(s[2].offset, 1.0);
        assert!(s.windows(2).all(|w| w[0].offset <= w[1].offset));
    }

    #[test]
    fn empty_stops_degrade_to_transparent() {
        let s = normalize_stops(&[]);
        assert_eq!(s.len(), 1);
        assert_eq!(sample_stops(&s, 0.5), Rgba::TRANSPARENT);
    }

    #[test]
    fn sampling_interpolates_and_clamps() {
        let s = normalize_stops(&stops(&[(0.0, Rgba::BLACK), (1.0, Rgba::WHITE)]));
        assert_eq!(sample_stops(&s, 0.0), Rgba::BLACK);
        assert_eq!(sample_stops(&s, 1.0), Rgba::WHITE);
        assert_eq!(sample_stops(&s, 0.5), Rgba::rgb(128, 128, 128));
        assert_eq!(sample_stops(&s, -5.0), Rgba::BLACK);
        assert_eq!(sample_stops(&s, 5.0), Rgba::WHITE);
    }

    #[test]
    fn coincident_stops_make_a_hard_break() {
        let s = normalize_stops(&stops(&[
            (0.0, Rgba::BLACK),
            (0.5, Rgba::BLACK),
            (0.5, Rgba::WHITE),
            (1.0, Rgba::WHITE),
        ]));
        assert_eq!(sample_stops(&s, 0.25), Rgba::BLACK);
        assert_eq!(sample_stops(&s, 0.75), Rgba::WHITE);
    }

    #[test]
    fn linear_gradient_runs_left_to_right_at_zero_degrees() {
        let rect = Rect::new(0.0, 0.0, 100.0, 100.0);
        assert!(linear_gradient_t(&rect, 0.0, 0.0, 50.0) < 0.01);
        assert!((linear_gradient_t(&rect, 0.0, 50.0, 50.0) - 0.5).abs() < 0.01);
        assert!(linear_gradient_t(&rect, 0.0, 100.0, 50.0) > 0.99);
        assert_eq!(
            linear_gradient_t(&rect, 0.0, 30.0, 0.0),
            linear_gradient_t(&rect, 0.0, 30.0, 100.0)
        );
    }

    #[test]
    fn linear_gradient_runs_top_to_bottom_at_ninety_degrees() {
        let rect = Rect::new(0.0, 0.0, 100.0, 100.0);
        assert!(linear_gradient_t(&rect, 90.0, 50.0, 0.0) < 0.01);
        assert!(linear_gradient_t(&rect, 90.0, 50.0, 100.0) > 0.99);
    }

    #[test]
    fn radial_gradient_grows_from_the_center() {
        let rect = Rect::new(0.0, 0.0, 100.0, 100.0);
        assert_eq!(radial_gradient_t(&rect, 50.0, 50.0), 0.0);
        assert!((radial_gradient_t(&rect, 0.0, 0.0) - 1.0).abs() < 0.001);
        let mid = radial_gradient_t(&rect, 50.0, 0.0);
        assert!(mid > 0.6 && mid < 0.8, "{mid}");
    }

    #[test]
    fn degenerate_rects_do_not_divide_by_zero() {
        let flat = Rect::new(0.0, 0.0, 0.0, 0.0);
        assert!(linear_gradient_t(&flat, 45.0, 0.0, 0.0).is_finite());
        assert!(radial_gradient_t(&flat, 0.0, 0.0).is_finite());
    }
}
