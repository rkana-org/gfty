use std::{
    fs,
    path::{Path, PathBuf},
    sync::mpsc,
    time::Duration,
};

use anyhow::{Context, Result, bail};
use notify::{Event, EventKind, RecursiveMode, Watcher};

pub fn watch_label(
    label_path: &Path,
    svg_output: Option<&Path>,
    json_output: Option<&Path>,
    system_fonts: bool,
) -> Result<()> {
    if svg_output.is_none() && json_output.is_none() {
        bail!("watch needs at least one of --svg or --json");
    }

    let initial = crate::config::LoadedLabel::load(label_path)?;
    let project_root = initial.project_root.clone();
    let mut inputs = watch_inputs(&initial)?;
    rebuild(label_path, svg_output, json_output, system_fonts)?;

    let ignored_outputs: Vec<_> = [svg_output, json_output]
        .into_iter()
        .flatten()
        .map(absolute_path)
        .collect::<Result<_>>()?;
    let (sender, receiver) = mpsc::channel();
    let mut watcher = notify::recommended_watcher(move |event| {
        let _ = sender.send(event);
    })
    .context("failed to create filesystem watcher")?;
    watcher
        .watch(&project_root, RecursiveMode::Recursive)
        .with_context(|| format!("failed to watch {}", project_root.display()))?;

    eprintln!("watching {}", project_root.display());
    loop {
        let first = receiver.recv().context("filesystem watcher stopped")?;
        let mut events = vec![first];
        while let Ok(event) = receiver.recv_timeout(Duration::from_millis(150)) {
            events.push(event);
        }

        let mut relevant = false;
        for event in events {
            match event {
                Ok(event) => {
                    if event_is_relevant(&event, &inputs, &ignored_outputs) {
                        relevant = true;
                    }
                }
                Err(error) => eprintln!("watch error: {error}"),
            }
        }
        if !relevant {
            continue;
        }

        match rebuild(label_path, svg_output, json_output, system_fonts) {
            Ok(()) => {
                if let Ok(label) = crate::config::LoadedLabel::load(label_path)
                    && let Ok(updated) = watch_inputs(&label)
                {
                    inputs = updated;
                }
                eprintln!("rebuilt");
            }
            Err(error) => eprintln!("rebuild failed: {error:#}"),
        }
    }
}

fn rebuild(
    label_path: &Path,
    svg_output: Option<&Path>,
    json_output: Option<&Path>,
    system_fonts: bool,
) -> Result<()> {
    let label = crate::config::LoadedLabel::load(label_path)?;
    let rendered = crate::compose::render_label(&label, system_fonts)?;
    if let Some(path) = svg_output {
        fs::write(path, &rendered.svg)
            .with_context(|| format!("failed to write SVG {}", path.display()))?;
    }
    if let Some(path) = json_output {
        let document = crate::export::export_rendered(&rendered)?;
        let mut json = serde_json::to_vec(&document)?;
        json.push(b'\n');
        fs::write(path, json)
            .with_context(|| format!("failed to write JSON {}", path.display()))?;
    }
    Ok(())
}

#[derive(Debug)]
struct WatchInputs {
    files: Vec<PathBuf>,
    directories: Vec<PathBuf>,
}

fn watch_inputs(label: &crate::config::LoadedLabel) -> Result<WatchInputs> {
    let template = label.template_path();
    let mut files = vec![absolute_path(&label.path)?, absolute_path(&template)?];
    files.push(absolute_path(&template.with_extension("toml"))?);
    for definition in label.config.icon.values() {
        let icon = label.icon_path(definition);
        files.push(absolute_path(&icon)?);
        files.push(absolute_path(&icon.with_extension("toml"))?);
    }
    let fonts = label.project_root.join("fonts");
    let directories = vec![absolute_path(&fonts)?];
    Ok(WatchInputs { files, directories })
}

fn event_is_relevant(event: &Event, inputs: &WatchInputs, ignored_outputs: &[PathBuf]) -> bool {
    if matches!(event.kind, EventKind::Access(_)) {
        return false;
    }
    event.paths.iter().any(|path| {
        let path = absolute_path(path).unwrap_or_else(|_| path.to_owned());
        !ignored_outputs.contains(&path)
            && (inputs.files.contains(&path)
                || inputs
                    .directories
                    .iter()
                    .any(|directory| path.starts_with(directory)))
    })
}

fn absolute_path(path: &Path) -> Result<PathBuf> {
    if path.exists() {
        return path
            .canonicalize()
            .with_context(|| format!("failed to resolve {}", path.display()));
    }
    if path.is_absolute() {
        Ok(path.to_owned())
    } else {
        Ok(std::env::current_dir()
            .context("failed to determine current directory")?
            .join(path))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use notify::event::{CreateKind, ModifyKind};

    #[test]
    fn ignores_access_and_generated_output_events() {
        let output = absolute_path(Path::new("preview.svg")).unwrap();
        let generated = Event {
            kind: EventKind::Modify(ModifyKind::Any),
            paths: vec![output.clone()],
            attrs: Default::default(),
        };
        let inputs = WatchInputs {
            files: vec![absolute_path(Path::new("source.svg")).unwrap()],
            directories: Vec::new(),
        };
        assert!(!event_is_relevant(&generated, &inputs, &[output]));

        let access = Event {
            kind: EventKind::Access(notify::event::AccessKind::Any),
            paths: vec![PathBuf::from("source.svg")],
            attrs: Default::default(),
        };
        assert!(!event_is_relevant(&access, &inputs, &[]));

        let source = Event {
            kind: EventKind::Create(CreateKind::File),
            paths: vec![PathBuf::from("source.svg")],
            attrs: Default::default(),
        };
        assert!(event_is_relevant(&source, &inputs, &[]));
    }
}
