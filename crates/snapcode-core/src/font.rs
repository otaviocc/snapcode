// SPDX-License-Identifier: MIT
//! Font loading and family resolution.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use cosmic_text::fontdb;
use cosmic_text::{Attrs, Family, FontSystem, Style, Weight};

use crate::config::FontConfig;

pub const EMBEDDED_FAMILY: &str = "JetBrains Mono";

const EMBEDDED_FACES: &[(&str, &[u8])] = &[
    (
        "Regular",
        include_bytes!("../assets/fonts/JetBrainsMono-Regular.ttf"),
    ),
    (
        "Bold",
        include_bytes!("../assets/fonts/JetBrainsMono-Bold.ttf"),
    ),
    (
        "Italic",
        include_bytes!("../assets/fonts/JetBrainsMono-Italic.ttf"),
    ),
    (
        "BoldItalic",
        include_bytes!("../assets/fonts/JetBrainsMono-BoldItalic.ttf"),
    ),
];

#[derive(Debug, thiserror::Error)]
pub enum FontError {
    #[error(
        "font family {requested:?} not found; the embedded {EMBEDDED_FAMILY:?} is always available"
    )]
    FamilyNotFound { requested: String },
    #[error("failed to read font file {}: {source}", path.display())]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

pub struct FontStyles {
    family: String,
    allow_bold: bool,
    allow_italic: bool,
    features: cosmic_text::FontFeatures,
}

impl FontStyles {
    pub fn family(&self) -> &str {
        &self.family
    }

    pub fn attrs(&self, bold: bool, italic: bool) -> Attrs<'_> {
        let mut attrs = Attrs::new()
            .family(Family::Name(&self.family))
            .font_features(self.features.clone());
        if bold && self.allow_bold {
            attrs = attrs.weight(Weight::BOLD);
        }
        if italic && self.allow_italic {
            attrs = attrs.style(Style::Italic);
        }
        attrs
    }
}

pub struct FontStack {
    system: FontSystem,
    styles: FontStyles,
}

impl FontStack {
    pub fn new(config: &FontConfig) -> Result<Self, FontError> {
        Self::build(config, true)
    }

    pub fn embedded_only(config: &FontConfig) -> Result<Self, FontError> {
        Self::build(config, false)
    }

    fn build(config: &FontConfig, load_system: bool) -> Result<Self, FontError> {
        let mut db = fontdb::Database::new();
        if load_system {
            db.load_system_fonts();
        }
        for (_, bytes) in EMBEDDED_FACES {
            db.load_font_source(fontdb::Source::Binary(Arc::new(*bytes)));
        }
        for path in &config.extra_font_files {
            let data = std::fs::read(path).map_err(|source| FontError::Io {
                path: path.clone(),
                source,
            })?;
            db.load_font_source(fontdb::Source::Binary(Arc::new(data)));
        }

        let family = match config.family.as_deref().map(str::trim) {
            Some(name) if !name.is_empty() => {
                if !family_exists(&db, name) {
                    return Err(FontError::FamilyNotFound {
                        requested: name.to_string(),
                    });
                }
                name.to_string()
            }
            _ => EMBEDDED_FAMILY.to_string(),
        };

        let system = FontSystem::new_with_locale_and_db("en-US".to_string(), db);

        let mut features = cosmic_text::FontFeatures::new();
        if config.ligatures {
            features.enable(cosmic_text::FeatureTag::new(b"calt"));
            features.enable(cosmic_text::FeatureTag::new(b"liga"));
        } else {
            features.disable(cosmic_text::FeatureTag::new(b"calt"));
            features.disable(cosmic_text::FeatureTag::new(b"liga"));
        }

        Ok(Self {
            system,
            styles: FontStyles {
                family,
                allow_bold: config.allow_bold,
                allow_italic: config.allow_italic,
                features,
            },
        })
    }

    pub fn system_mut(&mut self) -> &mut FontSystem {
        &mut self.system
    }

    pub fn styles(&self) -> &FontStyles {
        &self.styles
    }

    pub fn split(&mut self) -> (&mut FontSystem, &FontStyles) {
        (&mut self.system, &self.styles)
    }

    pub fn family(&self) -> &str {
        self.styles.family()
    }

    pub fn attrs(&self, bold: bool, italic: bool) -> Attrs<'_> {
        self.styles.attrs(bold, italic)
    }

    pub fn available_monospace_families(&self) -> Vec<String> {
        let mut names: Vec<String> = self
            .system
            .db()
            .faces()
            .filter(|f| f.monospaced)
            .filter_map(|f| f.families.first().map(|(name, _)| name.clone()))
            .collect();
        names.sort_unstable();
        names.dedup();
        names
    }
}

fn family_exists(db: &fontdb::Database, name: &str) -> bool {
    let wanted = name.to_ascii_lowercase();
    db.faces().any(|face| {
        face.families
            .iter()
            .any(|(family, _)| family.to_ascii_lowercase() == wanted)
    })
}

pub fn family_name_of_file(path: &Path) -> Result<Option<String>, FontError> {
    let data = std::fs::read(path).map_err(|source| FontError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let mut db = fontdb::Database::new();
    db.load_font_source(fontdb::Source::Binary(Arc::new(data)));
    let name = db
        .faces()
        .next()
        .and_then(|f| f.families.first().map(|(n, _)| n.clone()));
    Ok(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_family_name_is_correct() {
        let mut db = fontdb::Database::new();
        for (face, bytes) in EMBEDDED_FACES {
            db.load_font_source(fontdb::Source::Binary(Arc::new(*bytes)));
            assert!(!bytes.is_empty(), "{face} face is empty");
        }
        assert!(
            family_exists(&db, EMBEDDED_FAMILY),
            "embedded fonts do not provide {EMBEDDED_FAMILY:?}; found: {:?}",
            db.faces()
                .filter_map(|f| f.families.first().map(|(n, _)| n.clone()))
                .collect::<Vec<_>>()
        );
        assert_eq!(db.faces().count(), 4, "expected four embedded faces");
    }

    #[test]
    fn embedded_only_stack_needs_no_system_fonts() {
        let stack = FontStack::embedded_only(&FontConfig::default()).unwrap();
        assert_eq!(stack.family(), EMBEDDED_FAMILY);
    }

    #[test]
    fn unknown_family_is_rejected_rather_than_silently_substituted() {
        let config = FontConfig {
            family: Some("No Such Font 12345".into()),
            ..Default::default()
        };
        assert!(matches!(
            FontStack::embedded_only(&config),
            Err(FontError::FamilyNotFound { .. })
        ));
    }

    #[test]
    fn explicit_family_is_honored_when_present() {
        let config = FontConfig {
            family: Some(EMBEDDED_FAMILY.into()),
            ..Default::default()
        };
        let stack = FontStack::embedded_only(&config).unwrap();
        assert_eq!(stack.family(), EMBEDDED_FAMILY);
    }

    #[test]
    fn family_matching_ignores_case() {
        let config = FontConfig {
            family: Some("jetbrains mono".into()),
            ..Default::default()
        };
        assert!(FontStack::embedded_only(&config).is_ok());
    }

    #[test]
    fn bold_and_italic_can_be_disabled() {
        let config = FontConfig {
            allow_bold: false,
            allow_italic: false,
            ..Default::default()
        };
        let stack = FontStack::embedded_only(&config).unwrap();
        let attrs = stack.attrs(true, true);
        assert_eq!(attrs.weight, Weight::NORMAL);
        assert_eq!(attrs.style, Style::Normal);

        let stack = FontStack::embedded_only(&FontConfig::default()).unwrap();
        let attrs = stack.attrs(true, true);
        assert_eq!(attrs.weight, Weight::BOLD);
        assert_eq!(attrs.style, Style::Italic);
    }

    #[test]
    fn missing_extra_font_file_is_reported() {
        let config = FontConfig {
            extra_font_files: vec![PathBuf::from("/nonexistent/font.ttf")],
            ..Default::default()
        };
        assert!(matches!(
            FontStack::embedded_only(&config),
            Err(FontError::Io { .. })
        ));
    }
}
