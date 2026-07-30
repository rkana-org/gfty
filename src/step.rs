use std::{
    collections::BTreeMap,
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use anyhow::{Context, Result, bail};

static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

pub fn expected_filament_names(filaments: &[u32]) -> Vec<String> {
    let width = filaments
        .iter()
        .max()
        .map_or(1, |filament| filament.to_string().len());
    filaments
        .iter()
        .map(|filament| format!("part-{filament:0width$}"))
        .collect()
}

pub fn validate_label_step(contents: &[u8], filaments: &[u32]) -> Result<()> {
    let source = std::str::from_utf8(contents).context("downloaded STEP is not UTF-8 text")?;
    if !source.starts_with("ISO-10303-21;") {
        bail!("downloaded file is not an ISO-10303-21 STEP file");
    }

    let expected = expected_filament_names(filaments);
    validate_records(source, "PRODUCT('", "product", &expected)?;
    validate_records(source, "MANIFOLD_SOLID_BREP('", "solid body", &expected)?;
    Ok(())
}

fn validate_records(
    source: &str,
    marker: &str,
    description: &str,
    expected: &[String],
) -> Result<()> {
    let mut actual = Vec::new();
    for line in source.lines() {
        let Some(start) = line.find(marker) else {
            continue;
        };
        let quoted = &line[start + marker.len()..];
        actual.push(parse_step_string(quoted).with_context(|| {
            format!("downloaded STEP has a malformed {description} name record")
        })?);
    }

    let expected_counts = counts(expected.iter().cloned());
    let actual_counts = counts(actual.iter().cloned());
    if actual_counts != expected_counts {
        bail!(
            "Onshape generated unexpected {description}s; expected {}, received {}. Artwork may be disconnected from or outside the label blank",
            display_names(expected),
            display_names(&actual)
        );
    }
    Ok(())
}

fn parse_step_string(value: &str) -> Result<String> {
    let mut result = String::new();
    let mut chars = value.chars().peekable();
    while let Some(character) = chars.next() {
        if character != '\'' {
            result.push(character);
            continue;
        }
        if chars.peek() == Some(&'\'') {
            chars.next();
            result.push('\'');
        } else {
            return Ok(result);
        }
    }
    bail!("unterminated STEP string")
}

fn counts(values: impl IntoIterator<Item = String>) -> BTreeMap<String, usize> {
    let mut result = BTreeMap::new();
    for value in values {
        *result.entry(value).or_default() += 1;
    }
    result
}

fn display_names(names: &[String]) -> String {
    if names.is_empty() {
        "(none)".to_owned()
    } else {
        names.join(", ")
    }
}

pub fn ensure_output_available(path: &Path, force: bool) -> Result<()> {
    if path.exists() && !force {
        bail!(
            "output already exists: {}; use --force to replace it",
            path.display()
        );
    }
    let parent = output_parent(path);
    if !parent.is_dir() {
        bail!("output directory does not exist: {}", parent.display());
    }
    Ok(())
}

pub fn write_atomic(path: &Path, contents: &[u8], force: bool) -> Result<()> {
    ensure_output_available(path, force)?;
    let parent = output_parent(path);
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .context("output path must have a UTF-8 file name")?;

    let mut temporary = None;
    for _ in 0..100 {
        let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let candidate = parent.join(format!(
            ".{file_name}.gfty-{}-{sequence}.tmp",
            std::process::id()
        ));
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&candidate)
        {
            Ok(file) => {
                temporary = Some((candidate, file));
                break;
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => {
                return Err(error).with_context(|| {
                    format!("failed to create temporary output in {}", parent.display())
                });
            }
        }
    }
    let (temporary_path, mut file) =
        temporary.context("failed to allocate a temporary output file")?;

    let result = (|| -> Result<()> {
        file.write_all(contents).with_context(|| {
            format!(
                "failed to write temporary STEP {}",
                temporary_path.display()
            )
        })?;
        file.sync_all().with_context(|| {
            format!("failed to sync temporary STEP {}", temporary_path.display())
        })?;
        drop(file);
        replace_file(&temporary_path, path, force)
            .with_context(|| format!("failed to install STEP {}", path.display()))?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary_path);
    }
    result
}

fn output_parent(path: &Path) -> PathBuf {
    path.parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."))
        .to_owned()
}

#[cfg(unix)]
fn replace_file(source: &Path, destination: &Path, _force: bool) -> std::io::Result<()> {
    fs::rename(source, destination)
}

#[cfg(not(unix))]
fn replace_file(source: &Path, destination: &Path, force: bool) -> std::io::Result<()> {
    if force && destination.exists() {
        fs::remove_file(destination)?;
    }
    fs::rename(source, destination)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn step(products: &[&str], bodies: &[&str]) -> Vec<u8> {
        let mut source = "ISO-10303-21;\nDATA;\n".to_owned();
        for (index, name) in products.iter().enumerate() {
            source.push_str(&format!(
                "#{}=PRODUCT('{name}','{name}','{name}',());\n",
                index + 1
            ));
        }
        for (index, name) in bodies.iter().enumerate() {
            source.push_str(&format!(
                "#{}=MANIFOLD_SOLID_BREP('{name}',#1);\n",
                index + 100
            ));
        }
        source.push_str("ENDSEC;\nEND-ISO-10303-21;\n");
        source.into_bytes()
    }

    #[test]
    fn validates_exact_filament_products_and_bodies() {
        let contents = step(&["part-0", "part-3"], &["part-0", "part-3"]);
        validate_label_step(&contents, &[0, 3]).unwrap();
    }

    #[test]
    fn zero_pads_names_to_the_largest_filament_width() {
        assert_eq!(
            expected_filament_names(&[0, 2, 10]),
            ["part-00", "part-02", "part-10"]
        );
    }

    #[test]
    fn rejects_unexpected_disconnected_parts() {
        let contents = step(
            &["part-0", "part-1", "Part 1"],
            &["part-0", "part-1", "Part 1"],
        );
        let error = validate_label_step(&contents, &[0, 1])
            .unwrap_err()
            .to_string();
        assert!(error.contains("Part 1"));
        assert!(error.contains("disconnected"));
    }

    #[test]
    fn writes_atomically_and_requires_force_for_replacement() {
        let temp = tempfile::tempdir().unwrap();
        let output = temp.path().join("label.step");
        write_atomic(&output, b"first", false).unwrap();
        assert_eq!(fs::read(&output).unwrap(), b"first");
        assert!(write_atomic(&output, b"second", false).is_err());
        write_atomic(&output, b"second", true).unwrap();
        assert_eq!(fs::read(&output).unwrap(), b"second");
        assert_eq!(fs::read_dir(temp.path()).unwrap().count(), 1);
    }
}
