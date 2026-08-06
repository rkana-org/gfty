use std::{
    fs,
    path::{Path, PathBuf},
    sync::mpsc,
    time::{Duration, Instant},
};

use anyhow::{Context, Result};
use colored::Colorize;
use notify::{Event, EventKind, RecursiveMode, Watcher};

pub fn watch_label(
    label_path: &Path,
    svg_output: Option<&Path>,
    font_options: &crate::svg::FontOptions,
    preview_options: crate::terminal_preview::PreviewOptions,
) -> Result<()> {
    let initial = crate::config::LoadedLabel::load(label_path)
        .with_context(|| format!("failed to load label {}", label_path.display()))?;
    let watch_root = initial.base_dir.clone();
    let mut inputs =
        watch_inputs(&initial, font_options).context("failed to collect watch inputs")?;

    let ignored_outputs: Vec<_> = svg_output
        .into_iter()
        .map(absolute_path)
        .collect::<Result<_>>()?;
    let (sender, receiver) = mpsc::channel();
    let mut watcher = notify::recommended_watcher(move |event| {
        let _ = sender.send(event);
    })
    .context("failed to create filesystem watcher")?;
    watch_input_directories(&mut watcher, &inputs)?;

    let mut preview = match crate::terminal_preview::PreviewSession::new(preview_options) {
        Ok(preview) => Some(preview),
        Err(error) => {
            eprintln!("{} {error:#}", "Preview unavailable".yellow().bold());
            None
        }
    };
    let mut rebuild_count = 1usize;
    let started = Instant::now();
    let initial_svg =
        rebuild(label_path, svg_output, font_options).context("initial watched build failed")?;
    redraw(
        preview.as_mut(),
        &initial_svg,
        label_path,
        &watch_root,
        rebuild_count,
        started.elapsed(),
    );

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
                Err(error) => eprintln!("{} {error}", "watch error:".red().bold()),
            }
        }
        if !relevant {
            continue;
        }

        rebuild_count += 1;
        let started = Instant::now();
        match rebuild(label_path, svg_output, font_options) {
            Ok(svg) => {
                if let Ok(label) = crate::config::LoadedLabel::load(label_path)
                    && let Ok(updated) = watch_inputs(&label, font_options)
                {
                    inputs = updated;
                }
                redraw(
                    preview.as_mut(),
                    &svg,
                    label_path,
                    &watch_root,
                    rebuild_count,
                    started.elapsed(),
                );
            }
            Err(error) => {
                clear_preview(preview.as_mut());
                print_watch_header(&watch_root);
                print_rebuild_status("Failed", rebuild_count, started.elapsed(), false);
                eprintln!("{} {error:#}", "error:".red().bold());
            }
        }
    }
}

fn rebuild(
    label_path: &Path,
    svg_output: Option<&Path>,
    font_options: &crate::svg::FontOptions,
) -> Result<String> {
    let label = crate::config::LoadedLabel::load(label_path)
        .with_context(|| format!("failed to reload label {}", label_path.display()))?;
    let rendered = crate::compose::render_label(&label, font_options)
        .with_context(|| format!("failed to render label {}", label.path.display()))?;
    if let Some(path) = svg_output {
        fs::write(path, &rendered.svg)
            .with_context(|| format!("failed to write SVG {}", path.display()))?;
    }
    Ok(rendered.svg)
}

fn redraw(
    preview: Option<&mut crate::terminal_preview::PreviewSession>,
    svg: &str,
    label_path: &Path,
    watch_root: &Path,
    rebuild_count: usize,
    elapsed: Duration,
) {
    let mut preview = preview;
    clear_preview(preview.as_deref_mut());
    print_watch_header(watch_root);
    print_rebuild_status("Rebuilt", rebuild_count, elapsed, true);
    if let Some(preview) = preview
        && let Err(error) = preview.show_svg(svg, &label_path.display().to_string(), false)
    {
        eprintln!("{} {error:#}", "Preview failed".yellow().bold());
    }
}

fn clear_preview(preview: Option<&mut crate::terminal_preview::PreviewSession>) {
    if let Some(preview) = preview
        && let Err(error) = preview.clear()
    {
        eprintln!("{} {error:#}", "Clear failed".yellow().bold());
    }
}

fn print_watch_header(watch_root: &Path) {
    eprintln!(
        "{} {}",
        "Watching".green().bold(),
        watch_root.display().to_string().bold()
    );
}

fn print_rebuild_status(action: &str, count: usize, elapsed: Duration, success: bool) {
    let action = if success {
        action.green().bold()
    } else {
        action.red().bold()
    };
    let timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    eprintln!(
        "{} {} {} in {}",
        action,
        format!("#{count}").bold(),
        timestamp.blue(),
        format_duration(elapsed).yellow()
    );
}

fn format_duration(duration: Duration) -> String {
    if duration.as_secs() > 0 {
        format!("{:.2}s", duration.as_secs_f64())
    } else {
        format!("{}ms", duration.as_millis())
    }
}

#[derive(Debug)]
struct WatchInputs {
    files: Vec<PathBuf>,
    directories: Vec<PathBuf>,
}

fn watch_inputs(
    label: &crate::config::LoadedLabel,
    font_options: &crate::svg::FontOptions,
) -> Result<WatchInputs> {
    let template = label.template_path();
    let mut files = vec![absolute_path(&label.path)?, absolute_path(&template)?];
    files.push(absolute_path(&template.with_extension("toml"))?);
    files.push(absolute_path(&label.bin_path())?);
    for definition in label.config.icon.values() {
        let icon = label.icon_path(definition);
        files.push(absolute_path(&icon)?);
        files.push(absolute_path(&icon.with_extension("toml"))?);
    }
    for entries in label.config.icons.values() {
        for entry in entries {
            if let crate::config::IconPlacement::Icon { icon, .. } = entry {
                let resolved = label
                    .resolve_icon(icon)
                    .with_context(|| format!("failed to resolve watched icon {icon:?}"))?;
                files.push(absolute_path(&resolved.path)?);
                files.push(absolute_path(&resolved.path.with_extension("toml"))?);
            }
        }
    }
    files.sort();
    files.dedup();
    let mut directories = font_options
        .directories
        .iter()
        .map(|path| absolute_path(path))
        .collect::<Result<Vec<_>>>()?;
    directories.sort();
    directories.dedup();
    Ok(WatchInputs { files, directories })
}

fn watch_input_directories(watcher: &mut impl Watcher, inputs: &WatchInputs) -> Result<()> {
    let mut parents = inputs
        .files
        .iter()
        .filter_map(|path| path.parent().map(Path::to_owned))
        .collect::<Vec<_>>();
    parents.sort();
    parents.dedup();
    for directory in parents {
        watcher
            .watch(&directory, RecursiveMode::NonRecursive)
            .with_context(|| format!("failed to watch {}", directory.display()))?;
    }
    for directory in &inputs.directories {
        if directory.is_dir() {
            watcher
                .watch(directory, RecursiveMode::Recursive)
                .with_context(|| format!("failed to watch {}", directory.display()))?;
        }
    }
    Ok(())
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
    let absolute = if path.is_absolute() {
        path.to_owned()
    } else {
        std::env::current_dir()
            .context("failed to determine current directory")?
            .join(path)
    };
    if let (Some(parent), Some(name)) = (absolute.parent(), absolute.file_name())
        && parent.exists()
    {
        return Ok(parent
            .canonicalize()
            .with_context(|| format!("failed to resolve {}", parent.display()))?
            .join(name));
    }
    Ok(absolute)
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
