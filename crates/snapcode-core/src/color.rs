// SPDX-License-Identifier: MIT
//! Color parsing and blending.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Rgba {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Rgba {
    pub const TRANSPARENT: Rgba = Rgba::new(0, 0, 0, 0);
    pub const BLACK: Rgba = Rgba::new(0, 0, 0, 255);
    pub const WHITE: Rgba = Rgba::new(255, 255, 255, 255);

    pub const fn new(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self::new(r, g, b, 255)
    }

    pub fn with_opacity(self, factor: f32) -> Self {
        let a = (self.a as f32 * factor.clamp(0.0, 1.0)).round();
        Self { a: a as u8, ..self }
    }

    pub const fn with_alpha(self, a: u8) -> Self {
        Self { a, ..self }
    }

    pub fn is_transparent(self) -> bool {
        self.a == 0
    }

    pub const fn opaque(self) -> Self {
        Self { a: 255, ..self }
    }

    pub fn lerp(self, other: Rgba, t: f32) -> Rgba {
        let t = t.clamp(0.0, 1.0);
        let mix = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * t).round() as u8;
        Rgba::new(
            mix(self.r, other.r),
            mix(self.g, other.g),
            mix(self.b, other.b),
            mix(self.a, other.a),
        )
    }

    pub fn over(self, bottom: Rgba) -> Rgba {
        if self.a == 255 {
            return self;
        }
        if self.a == 0 {
            return bottom;
        }
        let sa = self.a as f32 / 255.0;
        let ba = bottom.a as f32 / 255.0;
        let out_a = sa + ba * (1.0 - sa);
        if out_a <= f32::EPSILON {
            return Rgba::TRANSPARENT;
        }
        let mix = |s: u8, b: u8| {
            let v = (s as f32 * sa + b as f32 * ba * (1.0 - sa)) / out_a;
            v.round().clamp(0.0, 255.0) as u8
        };
        Rgba::new(
            mix(self.r, bottom.r),
            mix(self.g, bottom.g),
            mix(self.b, bottom.b),
            (out_a * 255.0).round() as u8,
        )
    }

    pub fn mix_rgb(self, target: Rgba, amount: f32) -> Rgba {
        let t = amount.clamp(0.0, 1.0);
        let mix = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * t).round() as u8;
        Rgba::new(
            mix(self.r, target.r),
            mix(self.g, target.g),
            mix(self.b, target.b),
            self.a,
        )
    }

    pub fn luminance(self) -> f32 {
        let channel = |c: u8| {
            let c = c as f32 / 255.0;
            if c <= 0.04045 {
                c / 12.92
            } else {
                ((c + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * channel(self.r) + 0.7152 * channel(self.g) + 0.0722 * channel(self.b)
    }

    pub fn is_dark(self) -> bool {
        self.luminance() < 0.5
    }

    pub fn to_hex(self) -> String {
        if self.a == 255 {
            format!("#{:02x}{:02x}{:02x}", self.r, self.g, self.b)
        } else {
            format!("#{:02x}{:02x}{:02x}{:02x}", self.r, self.g, self.b, self.a)
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("invalid color {input:?}: {reason}")]
pub struct ParseColorError {
    pub input: String,
    pub reason: &'static str,
}

impl FromStr for Rgba {
    type Err = ParseColorError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let raw = s.trim();
        let err = |reason: &'static str| ParseColorError {
            input: raw.to_string(),
            reason,
        };

        if raw.is_empty() {
            return Err(err("empty string"));
        }

        let lower = raw.to_ascii_lowercase();
        if let Some(named) = named_color(&lower) {
            return Ok(named);
        }

        if let Some(args) = lower
            .strip_prefix("rgba(")
            .or_else(|| lower.strip_prefix("rgb("))
        {
            let args = args.strip_suffix(')').ok_or_else(|| err("missing `)`"))?;
            let parts: Vec<&str> = args.split(',').map(str::trim).collect();
            if parts.len() != 3 && parts.len() != 4 {
                return Err(err("rgb() takes 3 or 4 components"));
            }
            let channel = |p: &str| p.parse::<u16>().ok().filter(|v| *v <= 255).map(|v| v as u8);
            let r = channel(parts[0]).ok_or_else(|| err("red out of range"))?;
            let g = channel(parts[1]).ok_or_else(|| err("green out of range"))?;
            let b = channel(parts[2]).ok_or_else(|| err("blue out of range"))?;
            let a = match parts.get(3) {
                Some(p) => parse_alpha(p).ok_or_else(|| err("alpha out of range"))?,
                None => 255,
            };
            return Ok(Rgba::new(r, g, b, a));
        }

        let hex = lower.strip_prefix('#').unwrap_or(&lower);
        if !hex.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(err("not a hex color or known name"));
        }
        let nib = |i: usize| u8::from_str_radix(&hex[i..i + 1], 16).unwrap();
        let byte = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).unwrap();
        match hex.len() {
            3 => Ok(Rgba::rgb(nib(0) * 17, nib(1) * 17, nib(2) * 17)),
            4 => Ok(Rgba::new(
                nib(0) * 17,
                nib(1) * 17,
                nib(2) * 17,
                nib(3) * 17,
            )),
            6 => Ok(Rgba::rgb(byte(0), byte(2), byte(4))),
            8 => Ok(Rgba::new(byte(0), byte(2), byte(4), byte(6))),
            _ => Err(err("hex must be 3, 4, 6, or 8 digits")),
        }
    }
}

fn parse_alpha(s: &str) -> Option<u8> {
    if s.contains('.') {
        let f = s.parse::<f32>().ok()?;
        (0.0..=1.0).contains(&f).then(|| (f * 255.0).round() as u8)
    } else {
        s.parse::<u16>().ok().filter(|v| *v <= 255).map(|v| v as u8)
    }
}

fn named_color(lower: &str) -> Option<Rgba> {
    let c = match lower {
        "transparent" | "none" => Rgba::TRANSPARENT,
        "black" => Rgba::BLACK,
        "white" => Rgba::WHITE,
        "red" => Rgba::rgb(255, 0, 0),
        "green" => Rgba::rgb(0, 128, 0),
        "blue" => Rgba::rgb(0, 0, 255),
        "yellow" => Rgba::rgb(255, 255, 0),
        "cyan" | "aqua" => Rgba::rgb(0, 255, 255),
        "magenta" | "fuchsia" => Rgba::rgb(255, 0, 255),
        "orange" => Rgba::rgb(255, 165, 0),
        "purple" => Rgba::rgb(128, 0, 128),
        "gray" | "grey" => Rgba::rgb(128, 128, 128),
        "silver" => Rgba::rgb(192, 192, 192),
        "navy" => Rgba::rgb(0, 0, 128),
        "teal" => Rgba::rgb(0, 128, 128),
        "olive" => Rgba::rgb(128, 128, 0),
        "maroon" => Rgba::rgb(128, 0, 0),
        "lime" => Rgba::rgb(0, 255, 0),
        _ => return None,
    };
    Some(c)
}

impl fmt::Display for Rgba {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_hex())
    }
}

impl Serialize for Rgba {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_hex())
    }
}

impl<'de> Deserialize<'de> for Rgba {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        s.parse().map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_hex_forms() {
        assert_eq!("#1a1a1a".parse::<Rgba>().unwrap(), Rgba::rgb(26, 26, 26));
        assert_eq!("1a1a1a".parse::<Rgba>().unwrap(), Rgba::rgb(26, 26, 26));
        assert_eq!("#fff".parse::<Rgba>().unwrap(), Rgba::WHITE);
        assert_eq!("#C15F3C".parse::<Rgba>().unwrap(), Rgba::rgb(193, 95, 60));
        assert_eq!(
            "#11223344".parse::<Rgba>().unwrap(),
            Rgba::new(17, 34, 51, 68)
        );
        assert_eq!("#f00f".parse::<Rgba>().unwrap(), Rgba::rgb(255, 0, 0));
    }

    #[test]
    fn parses_functional_and_named_forms() {
        assert_eq!("rgb(1, 2, 3)".parse::<Rgba>().unwrap(), Rgba::rgb(1, 2, 3));
        assert_eq!(
            "rgba(1,2,3,128)".parse::<Rgba>().unwrap(),
            Rgba::new(1, 2, 3, 128)
        );
        assert_eq!(
            "rgba(0,0,0,0.5)".parse::<Rgba>().unwrap(),
            Rgba::new(0, 0, 0, 128)
        );
        assert_eq!("transparent".parse::<Rgba>().unwrap(), Rgba::TRANSPARENT);
        assert_eq!("WHITE".parse::<Rgba>().unwrap(), Rgba::WHITE);
    }

    #[test]
    fn rejects_malformed_input() {
        for bad in ["", "#12345", "nope", "rgb(1,2)", "rgb(1,2,300)", "#gg0000"] {
            assert!(bad.parse::<Rgba>().is_err(), "{bad:?} should not parse");
        }
    }

    #[test]
    fn hex_roundtrips() {
        let opaque = Rgba::rgb(193, 95, 60);
        assert_eq!(opaque.to_hex(), "#c15f3c");
        assert_eq!(opaque.to_hex().parse::<Rgba>().unwrap(), opaque);

        let translucent = Rgba::new(0, 0, 0, 64);
        assert_eq!(translucent.to_hex(), "#00000040");
        assert_eq!(translucent.to_hex().parse::<Rgba>().unwrap(), translucent);
    }

    #[test]
    fn composites_over_background() {
        let bg = Rgba::WHITE;
        assert_eq!(Rgba::BLACK.over(bg), Rgba::BLACK);
        assert_eq!(Rgba::TRANSPARENT.over(bg), bg);
        let half = Rgba::new(0, 0, 0, 128).over(bg);
        assert!((half.r as i32 - 127).abs() <= 1);
        assert_eq!(half.a, 255);
    }

    #[test]
    fn opaque_keeps_the_color_and_drops_the_alpha() {
        let translucent = Rgba::new(10, 20, 30, 40);
        assert_eq!(translucent.opaque(), Rgba::rgb(10, 20, 30));
        let solid = Rgba::rgb(1, 2, 3);
        assert_eq!(solid.opaque(), solid);
    }

    #[test]
    fn luminance_separates_light_from_dark() {
        assert!(Rgba::rgb(26, 26, 26).is_dark());
        assert!(!Rgba::rgb(244, 243, 238).is_dark());
    }
}
