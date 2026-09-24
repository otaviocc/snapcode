// SPDX-License-Identifier: MIT
//! Rasterizes a [`Scene`] to RGBA pixels with `tiny-skia`.

use cosmic_text::{SwashCache, SwashContent};
use tiny_skia::{
    FillRule, FilterQuality, GradientStop as SkStop, LinearGradient, Paint as SkPaint, Path,
    PathBuilder, Pattern, Pixmap, Point, RadialGradient, Rect as SkRect, Shader, SpreadMode,
    Stroke, Transform,
};

use crate::color::Rgba;
use crate::font::FontStack;
use crate::scene::{
    sample_stops, Corners, ImageData, ImagePlacement, Op, Paint, PlacedGlyph, Rect, Scene,
};

const KAPPA: f32 = 0.552_284_8;

#[derive(Debug, Clone)]
pub struct Raster {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}

impl Raster {
    pub fn pixel(&self, x: u32, y: u32) -> Rgba {
        let i = ((y * self.width + x) * 4) as usize;
        Rgba::new(
            self.pixels[i],
            self.pixels[i + 1],
            self.pixels[i + 2],
            self.pixels[i + 3],
        )
    }
}

#[derive(Debug, thiserror::Error)]
pub enum RasterError {
    #[error("could not allocate a {width}x{height} image")]
    Allocation { width: u32, height: u32 },
}

pub fn render(scene: &Scene, fonts: &mut FontStack) -> Result<Raster, RasterError> {
    let mut pixmap = Pixmap::new(scene.width, scene.height).ok_or(RasterError::Allocation {
        width: scene.width,
        height: scene.height,
    })?;

    let mut cache = SwashCache::new();
    for op in &scene.ops {
        draw_op(&mut pixmap, op, fonts, &mut cache)?;
    }

    Ok(Raster {
        width: scene.width,
        height: scene.height,
        pixels: unpremultiply(&pixmap),
    })
}

fn draw_op(
    pixmap: &mut Pixmap,
    op: &Op,
    fonts: &mut FontStack,
    cache: &mut SwashCache,
) -> Result<(), RasterError> {
    match op {
        Op::Fill {
            rect,
            corners,
            paint,
        } => {
            let Some(path) = rounded_rect_path(rect, &corners.clamped(rect)) else {
                return Ok(());
            };
            let prepared = match paint {
                Paint::Image {
                    image,
                    placement,
                    blur,
                    darken,
                } => Some(prepare_image(image, rect, *placement, *blur, *darken)?),
                _ => None,
            };
            let shader = match &prepared {
                Some(pixmap) => Pattern::new(
                    pixmap.as_ref(),
                    SpreadMode::Pad,
                    FilterQuality::Bilinear,
                    1.0,
                    Transform::from_translate(rect.x, rect.y),
                ),
                None => build_shader(paint, rect),
            };
            let sk_paint = SkPaint {
                shader,
                anti_alias: true,
                ..Default::default()
            };
            pixmap.fill_path(
                &path,
                &sk_paint,
                FillRule::Winding,
                Transform::identity(),
                None,
            );
        }
        Op::Shadow {
            rect,
            corners,
            blur,
            color,
        } => draw_shadow(pixmap, rect, corners, *blur, *color)?,
        Op::Stroke {
            rect,
            corners,
            color,
            width,
        } => {
            let inset = rect.inflate(-width / 2.0);
            if let Some(path) = rounded_rect_path(&inset, &corners.clamped(&inset)) {
                let sk_paint = SkPaint {
                    shader: Shader::SolidColor(to_sk_color(*color)),
                    anti_alias: true,
                    ..Default::default()
                };
                pixmap.stroke_path(
                    &path,
                    &sk_paint,
                    &Stroke {
                        width: *width,
                        ..Default::default()
                    },
                    Transform::identity(),
                    None,
                );
            }
        }
        Op::Ellipse { rect, color } => {
            let mut pb = PathBuilder::new();
            pb.push_oval(
                SkRect::from_xywh(rect.x, rect.y, rect.width, rect.height)
                    .unwrap_or(SkRect::from_xywh(0.0, 0.0, 1.0, 1.0).unwrap()),
            );
            if let Some(path) = pb.finish() {
                let sk_paint = SkPaint {
                    shader: Shader::SolidColor(to_sk_color(*color)),
                    anti_alias: true,
                    ..Default::default()
                };
                pixmap.fill_path(
                    &path,
                    &sk_paint,
                    FillRule::Winding,
                    Transform::identity(),
                    None,
                );
            }
        }
        Op::Glyphs(glyphs) => draw_glyphs(pixmap, glyphs, fonts, cache),
    }
    Ok(())
}

fn rounded_rect_path(rect: &Rect, corners: &Corners) -> Option<Path> {
    if rect.is_empty() {
        return None;
    }
    if corners.is_square() {
        let mut pb = PathBuilder::new();
        pb.push_rect(SkRect::from_xywh(rect.x, rect.y, rect.width, rect.height)?);
        return pb.finish();
    }

    let (l, t, r, b) = (rect.x, rect.y, rect.right(), rect.bottom());
    let (tl, tr, br, bl) = (
        corners.top_left,
        corners.top_right,
        corners.bottom_right,
        corners.bottom_left,
    );

    let mut pb = PathBuilder::new();
    pb.move_to(l + tl, t);
    pb.line_to(r - tr, t);
    if tr > 0.0 {
        pb.cubic_to(r - tr + tr * KAPPA, t, r, t + tr - tr * KAPPA, r, t + tr);
    }
    pb.line_to(r, b - br);
    if br > 0.0 {
        pb.cubic_to(r, b - br + br * KAPPA, r - br + br * KAPPA, b, r - br, b);
    }
    pb.line_to(l + bl, b);
    if bl > 0.0 {
        pb.cubic_to(l + bl - bl * KAPPA, b, l, b - bl + bl * KAPPA, l, b - bl);
    }
    pb.line_to(l, t + tl);
    if tl > 0.0 {
        pb.cubic_to(l, t + tl - tl * KAPPA, l + tl - tl * KAPPA, t, l + tl, t);
    }
    pb.close();
    pb.finish()
}

fn build_shader<'a>(paint: &'a Paint, rect: &Rect) -> Shader<'a> {
    match paint {
        Paint::Solid(color) => Shader::SolidColor(to_sk_color(*color)),
        Paint::Linear { stops, angle } => {
            let (sin, cos) = angle.to_radians().sin_cos();
            let cx = rect.x + rect.width / 2.0;
            let cy = rect.y + rect.height / 2.0;
            let extent = (rect.width * cos.abs() + rect.height * sin.abs()) / 2.0;
            let start = Point::from_xy(cx - cos * extent, cy - sin * extent);
            let end = Point::from_xy(cx + cos * extent, cy + sin * extent);
            LinearGradient::new(
                start,
                end,
                to_sk_stops(stops),
                SpreadMode::Pad,
                Transform::identity(),
            )
            .unwrap_or_else(|| Shader::SolidColor(to_sk_color(sample_stops(stops, 0.0))))
        }
        Paint::Radial { stops } => {
            let cx = rect.x + rect.width / 2.0;
            let cy = rect.y + rect.height / 2.0;
            let radius = (rect.width.powi(2) + rect.height.powi(2)).sqrt() / 2.0;
            let center = Point::from_xy(cx, cy);
            RadialGradient::new(
                center,
                0.0,
                center,
                radius.max(f32::EPSILON),
                to_sk_stops(stops),
                SpreadMode::Pad,
                Transform::identity(),
            )
            .unwrap_or_else(|| Shader::SolidColor(to_sk_color(sample_stops(stops, 0.0))))
        }
        Paint::Image { .. } => Shader::SolidColor(to_sk_color(Rgba::TRANSPARENT)),
    }
}

fn prepare_image(
    image: &ImageData,
    rect: &Rect,
    placement: ImagePlacement,
    blur: f32,
    darken: f32,
) -> Result<Pixmap, RasterError> {
    let dest_w = rect.width.ceil().max(1.0) as u32;
    let dest_h = rect.height.ceil().max(1.0) as u32;
    let mut out = Pixmap::new(dest_w, dest_h).ok_or(RasterError::Allocation {
        width: dest_w,
        height: dest_h,
    })?;

    if image.width == 0 || image.height == 0 {
        return Ok(out);
    }

    let (sw, sh) = (image.width as f32, image.height as f32);
    let (dw, dh) = (dest_w as f32, dest_h as f32);
    let scale = match placement {
        ImagePlacement::Fill => (dw / sw).max(dh / sh),
        ImagePlacement::Fit => (dw / sw).min(dh / sh),
        ImagePlacement::Center | ImagePlacement::Tile => 1.0,
        ImagePlacement::Stretch => 1.0,
    };

    let data = image.pixels.as_slice();
    let sample = |sx: i32, sy: i32| -> Option<[u8; 4]> {
        if sx < 0 || sy < 0 || sx >= image.width as i32 || sy >= image.height as i32 {
            return None;
        }
        let i = ((sy as u32 * image.width + sx as u32) * 4) as usize;
        Some([data[i], data[i + 1], data[i + 2], data[i + 3]])
    };

    let mut rgba = vec![0u8; (dest_w * dest_h * 4) as usize];
    for y in 0..dest_h {
        for x in 0..dest_w {
            let (fx, fy) = (x as f32 + 0.5, y as f32 + 0.5);
            let texel = match placement {
                ImagePlacement::Stretch => sample((fx / dw * sw) as i32, (fy / dh * sh) as i32),
                ImagePlacement::Tile => sample(
                    (fx as i32).rem_euclid(image.width as i32),
                    (fy as i32).rem_euclid(image.height as i32),
                ),
                _ => {
                    let (scaled_w, scaled_h) = (sw * scale, sh * scale);
                    let ox = (dw - scaled_w) / 2.0;
                    let oy = (dh - scaled_h) / 2.0;
                    sample(((fx - ox) / scale) as i32, ((fy - oy) / scale) as i32)
                }
            };
            let Some(mut px) = texel else { continue };
            if darken != 0.0 {
                let target = if darken > 0.0 { 0.0_f32 } else { 255.0_f32 };
                let amount = darken.abs();
                for c in px.iter_mut().take(3) {
                    *c = (*c as f32 + (target - *c as f32) * amount).round() as u8;
                }
            }
            let i = ((y * dest_w + x) * 4) as usize;
            rgba[i..i + 4].copy_from_slice(&px);
        }
    }

    if blur > 0.5 {
        blur_rgba(&mut rgba, dest_w, dest_h, blur);
    }

    for chunk in rgba.as_chunks_mut::<4>().0 {
        let a = chunk[3] as u32;
        for c in chunk.iter_mut().take(3) {
            *c = ((*c as u32 * a + 127) / 255) as u8;
        }
    }
    out.data_mut().copy_from_slice(&rgba);
    Ok(out)
}

fn draw_shadow(
    pixmap: &mut Pixmap,
    rect: &Rect,
    corners: &Corners,
    blur: f32,
    color: Rgba,
) -> Result<(), RasterError> {
    let Some(path) = rounded_rect_path(rect, &corners.clamped(rect)) else {
        return Ok(());
    };

    if blur <= 0.5 {
        let sk_paint = SkPaint {
            shader: Shader::SolidColor(to_sk_color(color)),
            anti_alias: true,
            ..Default::default()
        };
        pixmap.fill_path(
            &path,
            &sk_paint,
            FillRule::Winding,
            Transform::identity(),
            None,
        );
        return Ok(());
    }

    let mut layer =
        Pixmap::new(pixmap.width(), pixmap.height()).ok_or(RasterError::Allocation {
            width: pixmap.width(),
            height: pixmap.height(),
        })?;
    let sk_paint = SkPaint {
        shader: Shader::SolidColor(to_sk_color(color)),
        anti_alias: true,
        ..Default::default()
    };
    layer.fill_path(
        &path,
        &sk_paint,
        FillRule::Winding,
        Transform::identity(),
        None,
    );

    let (canvas_w, canvas_h) = (layer.width(), layer.height());
    let reach = blur_reach(blur) + 2;
    let x0 = (rect.x.floor() as i64 - reach).clamp(0, canvas_w as i64) as u32;
    let y0 = (rect.y.floor() as i64 - reach).clamp(0, canvas_h as i64) as u32;
    let x1 = ((rect.x + rect.width).ceil() as i64 + reach).clamp(0, canvas_w as i64) as u32;
    let y1 = ((rect.y + rect.height).ceil() as i64 + reach).clamp(0, canvas_h as i64) as u32;
    if x1 > x0 && y1 > y0 {
        blur_region(layer.data_mut(), canvas_w, (x0, y0, x1 - x0, y1 - y0), blur);
    }

    pixmap.draw_pixmap(
        0,
        0,
        layer.as_ref(),
        &tiny_skia::PixmapPaint::default(),
        Transform::identity(),
        None,
    );
    Ok(())
}

fn blur_radius(sigma: f32) -> i32 {
    ((sigma * 0.5).round() as i32).clamp(1, 200)
}

fn blur_reach(sigma: f32) -> i64 {
    blur_radius(sigma) as i64 * 3
}

fn blur_rgba(data: &mut [u8], width: u32, height: u32, sigma: f32) {
    blur_region(data, width, (0, 0, width, height), sigma);
}

fn blur_region(data: &mut [u8], stride: u32, region: (u32, u32, u32, u32), sigma: f32) {
    let radius = blur_radius(sigma);
    let (x0, y0, w, h) = (
        region.0 as usize,
        region.1 as usize,
        region.2 as usize,
        region.3 as usize,
    );
    let stride = stride as usize;
    let mut scratch = vec![0u8; w.max(h) * 4];
    let mut copy = vec![0u8; w * h * 4];
    let mut sums = vec![0u32; w * 4];

    for _ in 0..3 {
        for y in y0..y0 + h {
            let start = (y * stride + x0) * 4;
            box_blur_row(&mut data[start..start + w * 4], &mut scratch, radius);
        }

        for (row, dst) in copy.chunks_exact_mut(w * 4).enumerate() {
            let start = ((y0 + row) * stride + x0) * 4;
            dst.copy_from_slice(&data[start..start + w * 4]);
        }
        box_blur_columns(&copy, data, (stride, x0, y0), (w, h), radius, &mut sums);
    }
}

fn box_blur_row(row: &mut [u8], line: &mut [u8], radius: i32) {
    let inner = (row.len() / 4) as i32;
    let window = (radius * 2 + 1) as u32;
    let mut sums = [0u32; 4];
    for k in -radius..=radius {
        let px = (k.clamp(0, inner - 1) as usize) * 4;
        for c in 0..4 {
            sums[c] += row[px + c] as u32;
        }
    }

    for i in 0..inner {
        let out = &mut line[(i as usize) * 4..(i as usize) * 4 + 4];
        for c in 0..4 {
            out[c] = (sums[c] / window) as u8;
        }
        let leaving = ((i - radius).clamp(0, inner - 1) as usize) * 4;
        let entering = ((i + radius + 1).clamp(0, inner - 1) as usize) * 4;
        for c in 0..4 {
            sums[c] = sums[c] + row[entering + c] as u32 - row[leaving + c] as u32;
        }
    }
    row.copy_from_slice(&line[..row.len()]);
}

fn box_blur_columns(
    source: &[u8],
    data: &mut [u8],
    (stride, x0, y0): (usize, usize, usize),
    (w, h): (usize, usize),
    radius: i32,
    sums: &mut [u32],
) {
    let window = (radius * 2 + 1) as u32;
    let row_of = |r: i32| -> &[u8] {
        let r = r.clamp(0, h as i32 - 1) as usize;
        &source[r * w * 4..(r + 1) * w * 4]
    };

    sums.fill(0);
    for k in -radius..=radius {
        for (sum, &v) in sums.iter_mut().zip(row_of(k)) {
            *sum += v as u32;
        }
    }

    for i in 0..h as i32 {
        let start = ((y0 + i as usize) * stride + x0) * 4;
        for (dst, &sum) in data[start..start + w * 4].iter_mut().zip(sums.iter()) {
            *dst = (sum / window) as u8;
        }
        let entering = row_of(i + radius + 1);
        let leaving = row_of(i - radius);
        for ((sum, &inc), &out) in sums.iter_mut().zip(entering).zip(leaving) {
            *sum = *sum + inc as u32 - out as u32;
        }
    }
}

fn draw_glyphs(
    pixmap: &mut Pixmap,
    glyphs: &[PlacedGlyph],
    fonts: &mut FontStack,
    cache: &mut SwashCache,
) {
    let (width, height) = (pixmap.width() as i32, pixmap.height() as i32);
    let system = fonts.system_mut();

    for placed in glyphs {
        let physical = placed
            .shaped
            .inner
            .physical((placed.origin_x, placed.origin_y), 1.0);
        let Some(image) = cache.get_image(system, physical.cache_key).as_ref() else {
            continue;
        };

        let left = physical.x + image.placement.left;
        let top = physical.y - image.placement.top;
        let (gw, gh) = (image.placement.width as i32, image.placement.height as i32);
        if gw <= 0 || gh <= 0 {
            continue;
        }

        let color = placed.shaped.color;
        let data = pixmap.data_mut();

        for gy in 0..gh {
            let y = top + gy;
            if y < 0 || y >= height {
                continue;
            }
            for gx in 0..gw {
                let x = left + gx;
                if x < 0 || x >= width {
                    continue;
                }

                let src = match image.content {
                    SwashContent::Mask | SwashContent::SubpixelMask => {
                        let a = image.data[(gy * gw + gx) as usize];
                        if a == 0 {
                            continue;
                        }
                        let alpha = (a as u32 * color.a as u32 + 127) / 255;
                        if alpha == 0 {
                            continue;
                        }
                        [color.r, color.g, color.b, alpha as u8]
                    }
                    SwashContent::Color => {
                        let i = ((gy * gw + gx) * 4) as usize;
                        let a = image.data[i + 3];
                        if a == 0 {
                            continue;
                        }
                        [image.data[i], image.data[i + 1], image.data[i + 2], a]
                    }
                };

                let dst = ((y * width + x) as usize) * 4;
                blend_premultiplied(&mut data[dst..dst + 4], src);
            }
        }
    }
}

fn blend_premultiplied(dst: &mut [u8], src: [u8; 4]) {
    let sa = src[3] as u32;
    if sa == 0 {
        return;
    }
    let inv = 255 - sa;
    for c in 0..3 {
        let s = (src[c] as u32 * sa + 127) / 255;
        dst[c] = (s + (dst[c] as u32 * inv + 127) / 255).min(255) as u8;
    }
    dst[3] = (sa + (dst[3] as u32 * inv + 127) / 255).min(255) as u8;
}

fn unpremultiply(pixmap: &Pixmap) -> Vec<u8> {
    let mut out = Vec::with_capacity(pixmap.data().len());
    for px in pixmap.pixels() {
        let a = px.alpha();
        if a == 0 {
            out.extend_from_slice(&[0, 0, 0, 0]);
            continue;
        }
        let un = |c: u8| ((c as u32 * 255 + a as u32 / 2) / a as u32).min(255) as u8;
        out.extend_from_slice(&[un(px.red()), un(px.green()), un(px.blue()), a]);
    }
    out
}

fn to_sk_color(c: Rgba) -> tiny_skia::Color {
    tiny_skia::Color::from_rgba8(c.r, c.g, c.b, c.a)
}

fn to_sk_stops(stops: &[crate::config::GradientStop]) -> Vec<SkStop> {
    stops
        .iter()
        .map(|s| SkStop::new(s.offset, to_sk_color(s.color)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn naive_pass(data: &mut [u8], w: i32, h: i32, radius: i32, horizontal: bool) {
        let source = data.to_vec();
        let (outer, inner) = if horizontal { (h, w) } else { (w, h) };
        let window = (radius * 2 + 1) as u32;
        for o in 0..outer {
            for i in 0..inner {
                let at = |i: i32| {
                    let (x, y) = if horizontal { (i, o) } else { (o, i) };
                    ((y * w + x) as usize) * 4
                };
                for c in 0..4 {
                    let sum: u32 = (-radius..=radius)
                        .map(|k| source[at((i + k).clamp(0, inner - 1)) + c] as u32)
                        .sum();
                    data[at(i) + c] = (sum / window) as u8;
                }
            }
        }
    }

    fn naive_blur(data: &mut [u8], w: u32, h: u32, sigma: f32) {
        let radius = blur_radius(sigma);
        for _ in 0..3 {
            naive_pass(data, w as i32, h as i32, radius, true);
            naive_pass(data, w as i32, h as i32, radius, false);
        }
    }

    fn block(w: u32, h: u32, (x0, y0, x1, y1): (u32, u32, u32, u32)) -> Vec<u8> {
        let mut data = vec![0u8; (w * h * 4) as usize];
        for y in y0..y1 {
            for x in x0..x1 {
                let i = ((y * w + x) * 4) as usize;
                data[i..i + 4].copy_from_slice(&[10 + (x % 7) as u8 * 20, 30, 200, 255]);
            }
        }
        data
    }

    #[test]
    fn the_fast_blur_matches_the_straightforward_one() {
        let (w, h) = (61, 47);
        let mut fast = block(w, h, (0, 5, 40, 47));
        let mut slow = fast.clone();
        blur_rgba(&mut fast, w, h, 9.0);
        naive_blur(&mut slow, w, h, 9.0);
        assert_eq!(fast, slow);
    }

    #[test]
    fn blurring_only_the_padded_region_changes_nothing() {
        let (w, h) = (90, 80);
        let sigma = 6.0;
        let mut full = block(w, h, (30, 25, 50, 45));
        let mut cropped = full.clone();
        blur_rgba(&mut full, w, h, sigma);

        let reach = blur_reach(sigma) as u32 + 2;
        blur_region(
            &mut cropped,
            w,
            (30 - reach, 25 - reach, 20 + 2 * reach, 20 + 2 * reach),
            sigma,
        );
        assert_eq!(full, cropped);
    }
}
