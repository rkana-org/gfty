use std::{
    collections::BTreeSet,
    env,
    path::Path,
    sync::{Arc, Mutex},
};

use anyhow::{Context, Result, bail};

/// Parse and normalize SVG into a path-oriented preview. Text is converted to
/// outlines when its font can be resolved from the deterministic font database.
pub fn normalize_svg(
    source: &str,
    resources_dir: &Path,
    project_root: &Path,
    system_fonts: bool,
) -> Result<String> {
    normalize_svg_with_prefix(source, resources_dir, project_root, system_fonts, None)
}

pub fn normalize_svg_with_prefix(
    source: &str,
    resources_dir: &Path,
    project_root: &Path,
    system_fonts: bool,
    id_prefix: Option<String>,
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
        let system_hint = if system_fonts {
            "install the font or add its files to the project's fonts/ directory"
        } else {
            "add its files to the project's fonts/ directory or pass --system-fonts"
        };
        bail!("requested font family is unavailable: {families}; {system_hint}");
    }
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
            temp.path(),
            false,
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
            temp.path(),
            false,
        )
        .unwrap_err();
        let message = format!("{error:#}");
        assert!(message.contains("Definitely Missing Font"), "{message}");
        assert!(message.contains("fonts/"), "{message}");
    }
}
