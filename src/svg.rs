use std::{env, path::Path};

use anyhow::{Context, Result};

/// Parse and normalize SVG into a path-oriented preview. Text is converted to
/// outlines when its font can be resolved from the deterministic font database.
pub fn normalize_svg(
    source: &str,
    resources_dir: &Path,
    project_root: &Path,
    system_fonts: bool,
) -> Result<String> {
    let mut options = usvg::Options {
        resources_dir: Some(resources_dir.to_owned()),
        ..usvg::Options::default()
    };

    let fontdb = options.fontdb_mut();
    let project_fonts = project_root.join("fonts");
    if project_fonts.is_dir() {
        fontdb.load_fonts_dir(&project_fonts);
    }
    if let Some(font_dirs) = env::var_os("GFTY_LABEL_FONT_DIRS") {
        for directory in env::split_paths(&font_dirs) {
            if directory.is_dir() {
                fontdb.load_fonts_dir(directory);
            }
        }
    }
    if system_fonts {
        fontdb.load_system_fonts();
    }

    let tree = usvg::Tree::from_str(source, &options).context("failed to normalize SVG")?;
    Ok(tree.to_string(&usvg::WriteOptions {
        preserve_text: false,
        coordinates_precision: 8,
        transforms_precision: 8,
        ..usvg::WriteOptions::default()
    }))
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
            temp.path(),
            false,
        )
        .unwrap();
        assert!(output.contains("<path"));
        assert!(!output.contains("<rect"));
    }
}
