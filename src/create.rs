use std::{
    collections::BTreeMap,
    env, fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail};

use crate::config::{
    ConfigKind, IconPlacement, LABEL_CONFIG_VERSION, LabelConfig, LoadedLabel, TextValue,
};

pub fn build_label(
    template: &Path,
    filament: u32,
    text: &[String],
    icons: &[String],
) -> Result<LoadedLabel> {
    let current_dir = env::current_dir().context("failed to determine current directory")?;
    let template = absolute_existing_path(template, &current_dir, "template")
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
        let source = absolute_existing_path(Path::new(source), &current_dir, "icon")
            .with_context(|| format!("failed to resolve icon {source:?}"))?;
        placements
            .entry(box_name.clone())
            .or_default()
            .push(IconPlacement::Icon {
                icon: path_to_toml_string(&source),
            });
    }

    Ok(LoadedLabel::from_config(
        LabelConfig {
            kind: Some(ConfigKind::Label),
            version: Some(LABEL_CONFIG_VERSION),
            template: path_to_toml_string(&template),
            bin: None,
            filament,
            text: text_fields,
            icon: BTreeMap::new(),
            icons: placements,
        },
        current_dir,
    ))
}

pub fn save_label(label: &LoadedLabel, path: &Path) -> Result<()> {
    let source = toml::to_string_pretty(&label.config)
        .context("failed to serialize created label as TOML")?;
    fs::write(path, source)
        .with_context(|| format!("failed to save created label TOML {}", path.display()))
}

fn pairs<'a>(values: &'a [String], option: &str) -> Result<impl Iterator<Item = &'a [String; 2]>> {
    let (pairs, remainder) = values.as_chunks::<2>();
    if !remainder.is_empty() {
        bail!("{option} requires exactly two values each time");
    }
    Ok(pairs.iter())
}

fn absolute_existing_path(path: &Path, current_dir: &Path, description: &str) -> Result<PathBuf> {
    let candidate = if path.is_absolute() {
        path.to_owned()
    } else {
        current_dir.join(path)
    };
    candidate
        .canonicalize()
        .with_context(|| format!("failed to resolve {description} {}", candidate.display()))
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
                kind: Some(ConfigKind::Label),
                version: Some(LABEL_CONFIG_VERSION),
                template: "basic.svg".to_owned(),
                bin: None,
                filament: 0,
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
        save_label(&label, &path).unwrap();
        let saved: LabelConfig = toml::from_str(&fs::read_to_string(path).unwrap()).unwrap();
        assert_eq!(saved.kind, Some(ConfigKind::Label));
        assert_eq!(saved.version, Some(LABEL_CONFIG_VERSION));
        assert_eq!(saved.template, "basic.svg");
        assert_eq!(saved.text["main"].content, "M{3}");
    }
}
