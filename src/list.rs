use std::{fs, path::Path};

use anyhow::{Context, Result};
use colored::Colorize;

#[derive(Debug)]
pub struct ProjectEntries {
    pub root: std::path::PathBuf,
    pub templates: Vec<String>,
    pub icons: Vec<String>,
    pub labels: Vec<String>,
}

pub fn discover(root: Option<&Path>) -> Result<ProjectEntries> {
    let root = match root {
        Some(root) if root.is_absolute() => root.to_owned(),
        Some(root) => std::env::current_dir()
            .context("failed to determine current directory")?
            .join(root),
        None => std::env::current_dir().context("failed to determine current directory")?,
    };
    discover_from(&root)
}

fn discover_from(root: &Path) -> Result<ProjectEntries> {
    Ok(ProjectEntries {
        root: root.to_owned(),
        templates: collect(&root.join("templates"), "svg")?
            .into_iter()
            .map(|path| format!("templates/{path}"))
            .collect(),
        icons: collect(&root.join("icons"), "svg")?
            .into_iter()
            .map(|path| format!("icons/{path}"))
            .collect(),
        labels: collect(&root.join("labels"), "toml")?
            .into_iter()
            .map(|path| format!("labels/{path}"))
            .collect(),
    })
}

fn collect(directory: &Path, extension: &str) -> Result<Vec<String>> {
    if !directory.exists() {
        return Ok(Vec::new());
    }
    let mut result = Vec::new();
    collect_recursive(directory, directory, extension, &mut result)?;
    result.sort();
    Ok(result)
}

fn collect_recursive(
    root: &Path,
    directory: &Path,
    extension: &str,
    result: &mut Vec<String>,
) -> Result<()> {
    let entries = fs::read_dir(directory)
        .with_context(|| format!("failed to read directory {}", directory.display()))?;
    for entry in entries {
        let entry =
            entry.with_context(|| format!("failed to read an entry in {}", directory.display()))?;
        let path = entry.path();
        let file_type = entry
            .file_type()
            .with_context(|| format!("failed to inspect {}", path.display()))?;
        if file_type.is_dir() {
            collect_recursive(root, &path, extension, result)?;
        } else if file_type.is_file()
            && path
                .extension()
                .and_then(|value| value.to_str())
                .is_some_and(|value| value.eq_ignore_ascii_case(extension))
        {
            let relative = path
                .strip_prefix(root)
                .expect("walked path remains below its root");
            result.push(relative.to_string_lossy().replace('\\', "/"));
        }
    }
    Ok(())
}

pub fn print_group(name: &str, values: &[String]) {
    println!("{}", format!("{name}:").bold().green());
    if values.is_empty() {
        println!("  {}", "(none)".dimmed());
    } else {
        for value in values {
            println!("  {value}");
        }
    }
}

pub fn print_templates(entries: &ProjectEntries, grouped: bool) {
    let path_indent = if grouped { "  " } else { "" };
    let detail_indent = if grouped { "    " } else { "  " };
    if entries.templates.is_empty() {
        println!("{path_indent}{}", "(none)".dimmed());
        return;
    }

    for value in &entries.templates {
        println!("{path_indent}{}", value.bold());
        match crate::template::TemplateInfo::load(&entries.root.join(value)) {
            Ok(info) => {
                println!(
                    "{detail_indent}{} {} × {} mm",
                    "size:".dimmed(),
                    compact_number(info.width_mm),
                    compact_number(info.height_mm)
                );
                print_names(
                    detail_indent,
                    "text:",
                    info.text_fields.keys().map(String::as_str),
                );
                print_icon_boxes(detail_indent, &info.icon_boxes);
            }
            Err(error) => println!(
                "{detail_indent}{} {error:#}",
                "invalid template:".red().bold()
            ),
        }
    }
}

fn print_names<'a>(indent: &str, label: &str, names: impl Iterator<Item = &'a str>) {
    let names: Vec<_> = names.collect();
    if names.is_empty() {
        println!("{indent}{} {}", label.dimmed(), "(none)".dimmed());
    } else {
        println!("{indent}{} {}", label.dimmed(), names.join(", ").bold());
    }
}

fn print_icon_boxes(
    indent: &str,
    boxes: &std::collections::BTreeMap<String, crate::template::IconBox>,
) {
    if boxes.is_empty() {
        println!("{indent}{} {}", "icon boxes:".dimmed(), "(none)".dimmed());
        return;
    }
    let values = boxes
        .iter()
        .map(|(name, icon_box)| {
            let direction = match icon_box.direction {
                crate::template::IconDirection::Horizontal => "horizontal",
                crate::template::IconDirection::Vertical => "vertical",
            };
            let alignment = match (icon_box.direction, icon_box.alignment) {
                (_, crate::template::IconAlignment::Center) => "center",
                (
                    crate::template::IconDirection::Horizontal,
                    crate::template::IconAlignment::Start,
                ) => "left",
                (
                    crate::template::IconDirection::Horizontal,
                    crate::template::IconAlignment::End,
                ) => "right",
                (
                    crate::template::IconDirection::Vertical,
                    crate::template::IconAlignment::Start,
                ) => "top",
                (crate::template::IconDirection::Vertical, crate::template::IconAlignment::End) => {
                    "bottom"
                }
            };
            format!("{name} ({direction}, {alignment})")
        })
        .collect::<Vec<_>>()
        .join(", ");
    println!("{indent}{} {}", "icon boxes:".dimmed(), values.bold());
}

fn compact_number(value: f64) -> String {
    let value = format!("{value:.6}");
    value.trim_end_matches('0').trim_end_matches('.').to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn returns_root_relative_prefixed_paths() {
        let temp = tempfile::tempdir().unwrap();
        for directory in ["templates", "icons", "labels"] {
            fs::create_dir(temp.path().join(directory)).unwrap();
        }
        fs::write(temp.path().join("templates/label.svg"), "").unwrap();
        fs::write(temp.path().join("icons/icon.svg"), "").unwrap();
        fs::write(temp.path().join("labels/example.toml"), "").unwrap();
        let entries = discover_from(temp.path()).unwrap();
        assert_eq!(entries.templates, ["templates/label.svg"]);
        assert_eq!(entries.icons, ["icons/icon.svg"]);
        assert_eq!(entries.labels, ["labels/example.toml"]);
    }

    #[test]
    fn recursively_collects_sorted_matching_files() {
        let temp = tempfile::tempdir().unwrap();
        fs::create_dir_all(temp.path().join("nested")).unwrap();
        fs::write(temp.path().join("z.svg"), "").unwrap();
        fs::write(temp.path().join("nested/a.SVG"), "").unwrap();
        fs::write(temp.path().join("ignore.toml"), "").unwrap();
        assert_eq!(
            collect(temp.path(), "svg").unwrap(),
            vec!["nested/a.SVG", "z.svg"]
        );
    }
}
