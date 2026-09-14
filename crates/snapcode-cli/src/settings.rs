// SPDX-License-Identifier: MIT
//! Configuration file discovery and layering.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use snapcode_core::config::RenderConfig;

const PROJECT_CONFIG: &str = "snapcode.toml";

pub fn config_dir() -> Option<PathBuf> {
    if cfg!(not(windows)) {
        if let Some(xdg) = std::env::var_os("XDG_CONFIG_HOME").filter(|v| !v.is_empty()) {
            return Some(PathBuf::from(xdg).join("snapcode"));
        }
        if let Some(home) = directories::BaseDirs::new() {
            return Some(home.home_dir().join(".config").join("snapcode"));
        }
    }
    directories::ProjectDirs::from("", "", "snapcode").map(|dirs| dirs.config_dir().to_path_buf())
}

pub fn config_path() -> Option<PathBuf> {
    config_dir().map(|dir| dir.join("config.toml"))
}

pub fn themes_dir() -> Option<PathBuf> {
    config_dir().map(|dir| dir.join("themes"))
}

pub fn load(explicit: Option<&Path>, no_config: bool) -> Result<RenderConfig> {
    if no_config {
        return Ok(RenderConfig::default());
    }

    if let Some(path) = explicit {
        return read_config(path);
    }

    let mut config = match config_path() {
        Some(path) if path.exists() => read_config(&path)?,
        _ => RenderConfig::default(),
    };

    let project = Path::new(PROJECT_CONFIG);
    if project.exists() {
        config = merge(config, project)?;
    }
    Ok(config)
}

fn read_config(path: &Path) -> Result<RenderConfig> {
    let text = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read {}", path.display()))?;
    toml::from_str(&text).with_context(|| format!("failed to parse {}", path.display()))
}

fn merge(base: RenderConfig, path: &Path) -> Result<RenderConfig> {
    let overlay_text = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read {}", path.display()))?;
    let overlay: toml::Value = toml::from_str(&overlay_text)
        .with_context(|| format!("failed to parse {}", path.display()))?;
    let base_value =
        toml::Value::try_from(&base).context("failed to serialize the base configuration")?;

    let merged = merge_values(base_value, overlay);
    merged
        .try_into()
        .with_context(|| format!("failed to apply {}", path.display()))
}

fn merge_values(base: toml::Value, overlay: toml::Value) -> toml::Value {
    match (base, overlay) {
        (toml::Value::Table(mut base), toml::Value::Table(overlay)) => {
            for (key, value) in overlay {
                let merged = match base.remove(&key) {
                    Some(existing) => merge_values(existing, value),
                    None => value,
                };
                base.insert(key, merged);
            }
            toml::Value::Table(base)
        }
        (_, overlay) => overlay,
    }
}

pub fn to_toml(config: &RenderConfig) -> Result<String> {
    toml::to_string_pretty(config).context("failed to serialize the configuration")
}

pub fn save(config: &RenderConfig) -> Result<PathBuf> {
    let path = config_path().context("could not determine a configuration directory")?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    std::fs::write(&path, to_toml(config)?)
        .with_context(|| format!("failed to write {}", path.display()))?;
    Ok(path)
}

#[cfg(test)]
#[allow(clippy::field_reassign_with_default)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("snapcode-settings-{name}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    #[cfg(not(windows))]
    fn config_lives_under_dot_config_on_unix() {
        let dir = config_dir().expect("a config directory must be resolvable");
        assert!(
            dir.ends_with("snapcode"),
            "config dir should be namespaced: {}",
            dir.display()
        );
        assert!(
            !dir.to_string_lossy().contains("Application Support"),
            "expected an XDG-style path, got {}",
            dir.display()
        );
    }

    #[test]
    fn no_config_yields_defaults() {
        let config = load(None, true).unwrap();
        assert_eq!(config, RenderConfig::default());
    }

    #[test]
    fn explicit_config_is_read() {
        let dir = temp_dir("explicit");
        let path = dir.join("c.toml");
        std::fs::write(&path, "scale = 4.0\n[window]\nradius = 2.0\n").unwrap();

        let config = load(Some(&path), false).unwrap();
        assert_eq!(config.scale, 4.0);
        assert_eq!(config.window.radius, 2.0);
        assert_eq!(config.dpi, RenderConfig::default().dpi);
    }

    #[test]
    fn a_missing_explicit_config_is_an_error() {
        let err = load(Some(Path::new("/nonexistent/snapcode.toml")), false).unwrap_err();
        assert!(
            err.to_string().contains("/nonexistent/snapcode.toml"),
            "{err}"
        );
    }

    #[test]
    fn a_malformed_config_names_the_file() {
        let dir = temp_dir("malformed");
        let path = dir.join("bad.toml");
        std::fs::write(&path, "scale = \"not a number\"\n").unwrap();
        let err = load(Some(&path), false).unwrap_err();
        assert!(err.to_string().contains("bad.toml"), "{err}");
    }

    #[test]
    fn overlays_merge_per_field_rather_than_replacing_tables() {
        let dir = temp_dir("merge");
        let overlay = dir.join("overlay.toml");
        std::fs::write(&overlay, "[window]\nradius = 99.0\n").unwrap();

        let mut base = RenderConfig::default();
        base.window.padding = 7.0;
        base.scale = 3.0;

        let merged = merge(base, &overlay).unwrap();
        assert_eq!(merged.window.radius, 99.0);
        assert_eq!(merged.window.padding, 7.0, "sibling field was lost");
        assert_eq!(merged.scale, 3.0, "unrelated section was lost");
    }

    #[test]
    fn merging_replaces_scalars_and_arrays_wholesale() {
        let base = toml::Value::try_from(RenderConfig::default()).unwrap();
        let overlay: toml::Value = toml::from_str("scale = 5.0").unwrap();
        let merged = merge_values(base, overlay);
        assert_eq!(merged.get("scale").unwrap().as_float().unwrap(), 5.0);
    }

    #[test]
    fn config_roundtrips_through_toml() {
        let mut config = RenderConfig::default();
        config.theme = "midnight".into();
        config.gutter.enabled = true;

        let text = to_toml(&config).unwrap();
        let parsed: RenderConfig = toml::from_str(&text).unwrap();
        assert_eq!(parsed, config);
    }
}
