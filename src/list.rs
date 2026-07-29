use std::{fs, path::Path};

use anyhow::{Context, Result};

#[derive(Debug)]
pub struct ProjectEntries {
    pub templates: Vec<String>,
    pub icons: Vec<String>,
    pub labels: Vec<String>,
}

pub fn discover() -> Result<ProjectEntries> {
    let current = std::env::current_dir().context("failed to determine current directory")?;
    let root = crate::config::find_project_root(&current)
        .with_context(|| format!("failed to find project root from {}", current.display()))?;
    Ok(ProjectEntries {
        templates: collect(&root.join("templates"), "svg")?,
        icons: collect(&root.join("icons"), "svg")?,
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
    println!("{name}:");
    if values.is_empty() {
        println!("  (none)");
    } else {
        for value in values {
            println!("  {value}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
