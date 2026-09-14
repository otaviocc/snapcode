// SPDX-License-Identifier: MIT
//! Renders syntax-highlighted code snippets into images.

pub mod backend;
pub mod color;
pub mod compose;
pub mod config;
pub mod encode;
pub mod font;
pub mod highlight;
pub mod layout;
pub mod scene;
pub mod theme;

use std::path::{Path, PathBuf};

use crate::backend::raster::Raster;
use crate::config::RenderConfig;
use crate::font::FontStack;
use crate::highlight::Highlighter;
use crate::scene::Scene;
use crate::theme::ThemeRegistry;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Config(#[from] config::ConfigError),
    #[error(transparent)]
    Theme(#[from] theme::ThemeError),
    #[error(transparent)]
    Font(#[from] font::FontError),
    #[error(transparent)]
    Highlight(#[from] highlight::HighlightError),
    #[error(transparent)]
    Compose(#[from] compose::ComposeError),
    #[error(transparent)]
    Raster(#[from] backend::raster::RasterError),
    #[error(transparent)]
    Encode(#[from] encode::EncodeError),
    #[error(transparent)]
    Svg(#[from] backend::svg::SvgError),
}

#[derive(Debug, Clone)]
pub struct RenderRequest<'a> {
    pub source: &'a str,
    pub path: Option<PathBuf>,
    pub config: &'a RenderConfig,
}

impl<'a> RenderRequest<'a> {
    pub fn new(source: &'a str, config: &'a RenderConfig) -> Self {
        Self {
            source,
            path: None,
            config,
        }
    }

    pub fn with_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.path = Some(path.into());
        self
    }
}

pub struct Renderer {
    highlighter: Highlighter,
    themes: ThemeRegistry,
    hermetic_fonts: bool,
}

impl Renderer {
    pub fn new() -> Self {
        Self {
            highlighter: Highlighter::new(),
            themes: ThemeRegistry::new(),
            hermetic_fonts: false,
        }
    }

    pub fn hermetic() -> Self {
        Self {
            hermetic_fonts: true,
            ..Self::new()
        }
    }

    pub fn load_themes(&mut self, dir: &Path) -> Result<(), Error> {
        self.themes.load_dir(dir)?;
        Ok(())
    }

    pub fn themes(&self) -> &ThemeRegistry {
        &self.themes
    }

    pub fn highlighter(&self) -> &Highlighter {
        &self.highlighter
    }

    pub fn compose(&self, request: &RenderRequest<'_>) -> Result<(Scene, FontStack), Error> {
        let config = request.config;
        config.validate()?;

        let chrome = self.themes.chrome(&config.theme)?;
        let syntax_theme_name = if config.syntax_theme.is_empty() {
            chrome.syntax_theme.as_str()
        } else {
            config.syntax_theme.as_str()
        };
        let syntax_theme = self.themes.syntax(syntax_theme_name)?;

        let syntax = self.highlighter.resolve(
            config.code.language.as_deref(),
            request.path.as_deref(),
            request.source,
        )?;
        let highlighted =
            self.highlighter
                .highlight(request.source, syntax, syntax_theme, &config.code)?;

        let mut fonts = if self.hermetic_fonts {
            FontStack::embedded_only(&config.font)?
        } else {
            FontStack::new(&config.font)?
        };

        let mut config = config.clone();
        if config.window.title.is_none() {
            config.window.title = request
                .path
                .as_ref()
                .and_then(|p| p.file_name())
                .and_then(|n| n.to_str())
                .map(str::to_string);
        }

        let shaped = layout::shape(&highlighted, &config, &mut fonts);
        let scene = compose::compose(&shaped, &config, chrome, highlighted.background, &mut fonts)?;
        Ok((scene, fonts))
    }

    pub fn render_raster(&self, request: &RenderRequest<'_>) -> Result<Raster, Error> {
        let (scene, mut fonts) = self.compose(request)?;
        Ok(backend::raster::render(&scene, &mut fonts)?)
    }

    pub fn render_png(&self, request: &RenderRequest<'_>) -> Result<Vec<u8>, Error> {
        let raster = self.render_raster(request)?;
        Ok(encode::to_png(&raster, request.config.dpi)?)
    }

    pub fn render_svg(&self, request: &RenderRequest<'_>) -> Result<(String, bool), Error> {
        let (scene, mut fonts) = self.compose(request)?;
        let dropped = backend::svg::image_backgrounds_dropped(&scene);
        let svg = backend::svg::render(&scene, &mut fonts)?;
        Ok((svg, dropped))
    }
}

impl Default for Renderer {
    fn default() -> Self {
        Self::new()
    }
}
