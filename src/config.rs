use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail};
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LabelConfig {
    pub template: String,

    #[serde(default)]
    pub text: BTreeMap<String, TextValue>,

    /// Locally named, reusable icon configurations.
    #[serde(default)]
    pub icon: BTreeMap<String, IconDefinition>,

    /// Ordered contents of each `icons-*` template box, keyed without the prefix.
    #[serde(default)]
    pub icons: BTreeMap<String, Vec<IconPlacement>>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TextValue {
    pub content: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IconDefinition {
    pub src: String,

    /// Keys are either resolved filament indices or exact source hex colors.
    #[serde(default)]
    pub colors: BTreeMap<String, u32>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum IconPlacement {
    Icon { icon: String },
    Spacer { spacer: String },
}

#[derive(Debug)]
pub struct LoadedLabel {
    pub path: PathBuf,
    pub project_root: PathBuf,
    pub config: LabelConfig,
}

impl LoadedLabel {
    pub fn load(path: &Path) -> Result<Self> {
        let path = path
            .canonicalize()
            .with_context(|| format!("failed to resolve label {}", path.display()))?;
        let source = fs::read_to_string(&path)
            .with_context(|| format!("failed to read label {}", path.display()))?;
        let config: LabelConfig = toml::from_str(&source)
            .with_context(|| format!("failed to parse label {}", path.display()))?;
        let project_root = find_project_root(path.parent().expect("label has a parent"))?;
        Ok(Self {
            path,
            project_root,
            config,
        })
    }

    pub fn template_path(&self) -> PathBuf {
        self.project_root
            .join("templates")
            .join(&self.config.template)
    }

    pub fn icon_path(&self, icon: &IconDefinition) -> PathBuf {
        self.project_root.join("icons").join(&icon.src)
    }

    pub fn validate(&self) -> Result<()> {
        ensure_file(&self.template_path(), "template")?;
        let template = crate::template::TemplateInfo::load(&self.template_path())?;

        for (field, value) in &self.config.text {
            if !template.text_fields.contains_key(field) {
                bail!("label config references unknown text field {field:?}");
            }
            crate::text::parse_colored_text(&value.content)
                .with_context(|| format!("invalid colored text in field {field:?}"))?;
        }

        for (name, icon) in &self.config.icon {
            ensure_file(&self.icon_path(icon), &format!("icon alias {name:?}"))?;
            validate_color_overrides(name, &icon.colors)?;
        }

        for (box_name, entries) in &self.config.icons {
            if box_name.starts_with("icons-") {
                bail!("icon box {box_name:?} must omit the template's `icons-` prefix");
            }
            if !template.icon_boxes.contains_key(box_name) {
                bail!("label config references unknown icon box {box_name:?}");
            }
            for entry in entries {
                match entry {
                    IconPlacement::Icon { icon } => {
                        if !self.config.icon.contains_key(icon) {
                            bail!("icon box {box_name:?} references unknown icon alias {icon:?}");
                        }
                    }
                    IconPlacement::Spacer { spacer } => {
                        crate::config::parse_length_mm(spacer)
                            .with_context(|| format!("invalid spacer in icon box {box_name:?}"))?;
                    }
                }
            }
        }

        Ok(())
    }
}

fn validate_color_overrides(name: &str, colors: &BTreeMap<String, u32>) -> Result<()> {
    let mut normalized = BTreeSet::new();
    for key in colors.keys() {
        let valid_index = key.parse::<u32>().is_ok();
        let valid_hex = normalize_hex_color(key).is_some();
        if !valid_index && !valid_hex {
            bail!(
                "icon alias {name:?} has invalid color override key {key:?}; expected an index or hex color"
            );
        }
        let canonical = normalize_hex_color(key).unwrap_or_else(|| key.clone());
        if !normalized.insert(canonical.clone()) {
            bail!("icon alias {name:?} repeats color override {canonical:?}");
        }
    }
    Ok(())
}

fn ensure_file(path: &Path, description: &str) -> Result<()> {
    if !path.is_file() {
        bail!("{description} does not exist: {}", path.display());
    }
    Ok(())
}

fn find_project_root(start: &Path) -> Result<PathBuf> {
    for directory in start.ancestors() {
        if directory.join("project.toml").is_file() {
            return Ok(directory.to_owned());
        }
    }
    bail!(
        "could not find project.toml in {} or any parent directory",
        start.display()
    )
}

pub fn normalize_hex_color(value: &str) -> Option<String> {
    let value = value.trim().to_ascii_lowercase();
    let digits = value.strip_prefix('#').unwrap_or(&value);
    match digits.len() {
        3 if digits.bytes().all(|c| c.is_ascii_hexdigit()) => {
            Some(digits.chars().flat_map(|c| [c, c]).collect::<String>())
        }
        6 if digits.bytes().all(|c| c.is_ascii_hexdigit()) => Some(digits.to_owned()),
        _ => None,
    }
}

pub fn parse_length_mm(value: &str) -> Result<f64> {
    let value = value.trim();
    let split = value
        .find(|c: char| !(c.is_ascii_digit() || matches!(c, '.' | '+' | '-' | 'e' | 'E')))
        .unwrap_or(value.len());
    let (number, unit) = value.split_at(split);
    let number: f64 = number
        .parse()
        .with_context(|| format!("invalid length number {number:?}"))?;
    let multiplier = match unit.trim().to_ascii_lowercase().as_str() {
        "" | "mm" => 1.0,
        "cm" => 10.0,
        "m" => 1000.0,
        "in" | "inch" | "inches" => 25.4,
        other => bail!("unsupported length unit {other:?}"),
    };
    let result = number * multiplier;
    if !result.is_finite() || result < 0.0 {
        bail!("length must be finite and non-negative");
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_hex_colors() {
        assert_eq!(normalize_hex_color("#F00").as_deref(), Some("ff0000"));
        assert_eq!(normalize_hex_color("00ff7F").as_deref(), Some("00ff7f"));
        assert_eq!(normalize_hex_color("nope"), None);
    }

    #[test]
    fn parses_lengths() {
        assert_eq!(parse_length_mm("2mm").unwrap(), 2.0);
        assert_eq!(parse_length_mm(" 2.5 cm ").unwrap(), 25.0);
        assert_eq!(parse_length_mm("1in").unwrap(), 25.4);
        assert!(parse_length_mm("-1mm").is_err());
    }
}
