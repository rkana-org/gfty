use std::{
    collections::BTreeMap,
    env,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail};

use crate::config::{IconDefinition, IconPlacement, LabelConfig, LoadedLabel, TextValue};

pub fn build_quick_label(
    template: &Path,
    text: &[String],
    icons: &[String],
) -> Result<LoadedLabel> {
    let current_dir = env::current_dir().context("failed to determine current directory")?;
    let project_root = crate::config::find_project_root(&current_dir)
        .with_context(|| format!("failed to find project root from {}", current_dir.display()))?;
    let template = project_relative_path(template, &project_root.join("templates"), "template")
        .with_context(|| format!("failed to resolve template {}", template.display()))?;

    let mut text_fields = BTreeMap::new();
    for pair in pairs(text, "--text")? {
        let [field, content] = pair;
        if text_fields
            .insert(
                field.clone(),
                TextValue {
                    content: content.clone(),
                },
            )
            .is_some()
        {
            bail!("--text specifies field {field:?} more than once");
        }
    }

    let mut definitions = BTreeMap::new();
    let mut placements = BTreeMap::<String, Vec<IconPlacement>>::new();
    for (index, pair) in pairs(icons, "--icon")?.enumerate() {
        let [box_name, source] = pair;
        let source = project_relative_path(Path::new(source), &project_root.join("icons"), "icon")
            .with_context(|| format!("failed to resolve icon {source:?}"))?;
        let alias = format!("quick-{index}");
        definitions.insert(
            alias.clone(),
            IconDefinition {
                src: path_to_toml_string(&source),
                colors: BTreeMap::new(),
            },
        );
        placements
            .entry(box_name.clone())
            .or_default()
            .push(IconPlacement::Icon { icon: alias });
    }

    Ok(LoadedLabel::from_config(
        LabelConfig {
            template: path_to_toml_string(&template),
            text: text_fields,
            icon: definitions,
            icons: placements,
        },
        project_root,
    ))
}

fn pairs<'a>(values: &'a [String], option: &str) -> Result<impl Iterator<Item = &'a [String; 2]>> {
    let (pairs, remainder) = values.as_chunks::<2>();
    if !remainder.is_empty() {
        bail!("{option} requires exactly two values each time");
    }
    Ok(pairs.iter())
}

fn project_relative_path(path: &Path, base: &Path, description: &str) -> Result<PathBuf> {
    let candidate = if path.is_absolute() {
        path.to_owned()
    } else if path.is_file() {
        path.canonicalize()
            .with_context(|| format!("failed to resolve {description} {}", path.display()))?
    } else {
        return Ok(path.to_owned());
    };
    let base = base.canonicalize().with_context(|| {
        format!(
            "failed to resolve {} directory {}",
            description,
            base.display()
        )
    })?;
    candidate
        .strip_prefix(&base)
        .map(Path::to_owned)
        .with_context(|| {
            format!(
                "{description} {} must be inside {}",
                candidate.display(),
                base.display()
            )
        })
}

fn path_to_toml_string(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_incomplete_pairs() {
        assert!(pairs(&["only-one".to_owned()], "--text").is_err());
    }

    #[test]
    fn normalizes_path_separators() {
        assert_eq!(
            path_to_toml_string(Path::new(r"foo\bar.svg")),
            "foo/bar.svg"
        );
    }
}
