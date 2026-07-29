use std::{
    collections::BTreeSet,
    env,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use anyhow::{Context, Result, bail};

/// Reusable SVG parser with the project's deterministic font database.
/// Constructing it scans font directories, so callers rendering a batch should
/// keep one parser for the complete batch.
#[derive(Debug, Clone, Default)]
pub struct FontOptions {
    pub system_fonts: bool,
    pub directories: Vec<PathBuf>,
}

pub struct SvgParser {
    fontdb: Arc<usvg::fontdb::Database>,
    system_fonts: bool,
}

impl SvgParser {
    pub fn new(font_options: &FontOptions) -> Self {
        let mut fontdb = usvg::fontdb::Database::new();
        if let Some(font_dirs) = env::var_os("GFTY_LABEL_FONT_DIRS") {
            for directory in env::split_paths(&font_dirs) {
                if directory.is_dir() {
                    fontdb.load_fonts_dir(directory);
                }
            }
        }
        for directory in &font_options.directories {
            if directory.is_dir() {
                fontdb.load_fonts_dir(directory);
            }
        }
        if font_options.system_fonts {
            fontdb.load_system_fonts();
        }
        Self {
            fontdb: Arc::new(fontdb),
            system_fonts: font_options.system_fonts,
        }
    }

    pub fn parse(&self, source: &str, resources_dir: &Path) -> Result<usvg::Tree> {
        let mut options = usvg::Options {
            resources_dir: Some(resources_dir.to_owned()),
            fontdb: Arc::clone(&self.fontdb),
            ..usvg::Options::default()
        };

        // usvg normally appends a generic serif fallback even when none of the
        // document's requested families exist. For labels, that turns misspelled or
        // unavailable fonts into missing/substituted geometry, so require an actual
        // match from the SVG's own CSS family list.
        let missing_families = Arc::new(Mutex::new(BTreeSet::new()));
        let missing_for_resolver = Arc::clone(&missing_families);
        options.font_resolver.select_font = Box::new(move |font, fontdb| {
            let families: Vec<_> = font
                .families()
                .iter()
                .map(|family| match family {
                    usvg::FontFamily::Serif => usvg::fontdb::Family::Serif,
                    usvg::FontFamily::SansSerif => usvg::fontdb::Family::SansSerif,
                    usvg::FontFamily::Cursive => usvg::fontdb::Family::Cursive,
                    usvg::FontFamily::Fantasy => usvg::fontdb::Family::Fantasy,
                    usvg::FontFamily::Monospace => usvg::fontdb::Family::Monospace,
                    usvg::FontFamily::Named(name) => usvg::fontdb::Family::Name(name),
                })
                .collect();
            let query = usvg::fontdb::Query {
                families: &families,
                weight: usvg::fontdb::Weight(font.weight()),
                stretch: font.stretch().into(),
                style: font.style().into(),
            };
            let selected = fontdb.query(&query);
            if selected.is_none()
                && let Ok(mut missing) = missing_for_resolver.lock()
            {
                missing.insert(
                    font.families()
                        .iter()
                        .map(font_family_name)
                        .collect::<Vec<_>>()
                        .join(", "),
                );
            }
            selected
        });

        let tree = usvg::Tree::from_str(source, &options).context("failed to normalize SVG")?;
        let missing_families = missing_families
            .lock()
            .map_err(|_| anyhow::anyhow!("failed to inspect requested fonts"))?;
        if !missing_families.is_empty() {
            let families = missing_families
                .iter()
                .map(|family| format!("{family:?}"))
                .collect::<Vec<_>>()
                .join(", ");
            let system_hint = if self.system_fonts {
                "install the font or add its files with --font-dir"
            } else {
                "add its files with --font-dir or pass --system-fonts"
            };
            bail!("requested font family is unavailable: {families}; {system_hint}");
        }
        drop(missing_families);
        Ok(tree)
    }
}

/// Parse and normalize SVG into a path-oriented preview. Text is converted to
/// outlines when its font can be resolved from the deterministic font database.
pub fn normalize_svg(
    source: &str,
    resources_dir: &Path,
    font_options: &FontOptions,
) -> Result<String> {
    normalize_svg_with_prefix(source, resources_dir, font_options, None)
}

pub fn normalize_svg_with_prefix(
    source: &str,
    resources_dir: &Path,
    font_options: &FontOptions,
    id_prefix: Option<String>,
) -> Result<String> {
    let parser = SvgParser::new(font_options);
    normalize_svg_with_parser(source, resources_dir, &parser, id_prefix)
}

pub fn normalize_svg_with_parser(
    source: &str,
    resources_dir: &Path,
    parser: &SvgParser,
    id_prefix: Option<String>,
) -> Result<String> {
    let tree = parser.parse(source, resources_dir)?;
    Ok(tree.to_string(&usvg::WriteOptions {
        id_prefix,
        preserve_text: false,
        coordinates_precision: 8,
        transforms_precision: 8,
        ..usvg::WriteOptions::default()
    }))
}

fn font_family_name(family: &usvg::FontFamily) -> String {
    match family {
        usvg::FontFamily::Serif => "serif".to_owned(),
        usvg::FontFamily::SansSerif => "sans-serif".to_owned(),
        usvg::FontFamily::Cursive => "cursive".to_owned(),
        usvg::FontFamily::Fantasy => "fantasy".to_owned(),
        usvg::FontFamily::Monospace => "monospace".to_owned(),
        usvg::FontFamily::Named(name) => name.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_primitives_to_geometry() {
        let temp = tempfile::tempdir().unwrap();
        let output = normalize_svg(
            r##"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10"><rect width="5" height="6" fill="#f00"/></svg>"##,
            temp.path(),
            &FontOptions::default(),
        )
        .unwrap();
        assert!(output.contains("<path"));
        assert!(!output.contains("<rect"));
    }

    #[test]
    fn rejects_unavailable_font_families() {
        let temp = tempfile::tempdir().unwrap();
        let error = normalize_svg(
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10"><text font-family="Definitely Missing Font">Test</text></svg>"#,
            temp.path(),
            &FontOptions::default(),
        )
        .unwrap_err();
        let message = format!("{error:#}");
        assert!(message.contains("Definitely Missing Font"), "{message}");
        assert!(message.contains("--font-dir"), "{message}");
    }
}
