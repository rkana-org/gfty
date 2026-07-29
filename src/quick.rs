use std::{
    collections::BTreeMap,
    env, fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail};

use crate::config::{IconPlacement, LabelConfig, LoadedLabel, TextValue};

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

    let mut placements = BTreeMap::<String, Vec<IconPlacement>>::new();
    for pair in pairs(icons, "--icon")? {
        let [box_name, source] = pair;
        let source = project_relative_path(Path::new(source), &project_root.join("icons"), "icon")
            .with_context(|| format!("failed to resolve icon {source:?}"))?;
        let reference = Path::new("icons").join(source);
        placements
            .entry(box_name.clone())
            .or_default()
            .push(IconPlacement::Icon {
                icon: path_to_toml_string(&reference),
            });
    }

    Ok(LoadedLabel::from_config(
        LabelConfig {
            template: path_to_toml_string(&template),
            text: text_fields,
            icon: BTreeMap::new(),
            icons: placements,
        },
        project_root,
    ))
}

pub fn save_quick_label(label: &LoadedLabel, path: &Path) -> Result<()> {
    let source =
        toml::to_string_pretty(&label.config).context("failed to serialize quick label as TOML")?;
    fs::write(path, source)
        .with_context(|| format!("failed to save quick label TOML {}", path.display()))
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

    #[test]
    fn saves_reusable_toml() {
        let temp = tempfile::tempdir().unwrap();
        let label = LoadedLabel::from_config(
            LabelConfig {
                template: "basic.svg".to_owned(),
                text: BTreeMap::from([(
                    "main".to_owned(),
                    TextValue {
                        content: "M{3}".to_owned(),
                    },
                )]),
                icon: BTreeMap::new(),
                icons: BTreeMap::new(),
            },
            temp.path().to_owned(),
        );
        let path = temp.path().join("saved.toml");
        save_quick_label(&label, &path).unwrap();
        let saved: LabelConfig = toml::from_str(&fs::read_to_string(path).unwrap()).unwrap();
        assert_eq!(saved.template, "basic.svg");
        assert_eq!(saved.text["main"].content, "M{3}");
    }
}
