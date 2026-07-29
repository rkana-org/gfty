use std::io::IsTerminal;

use anyhow::{Context, Result, bail};

use crate::cli::TerminalPreviewMode;

#[derive(Debug, Clone, Copy)]
pub struct PreviewOptions {
    pub mode: TerminalPreviewMode,
    pub width: u16,
}

impl PreviewOptions {
    pub fn enabled(self) -> bool {
        self.mode != TerminalPreviewMode::Never && self.width > 0 && std::io::stderr().is_terminal()
    }
}

pub fn show_svg(svg: &str, label: &str, options: PreviewOptions, clear: bool) -> Result<bool> {
    if !options.enabled() {
        return Ok(false);
    }

    #[cfg(unix)]
    {
        show_svg_unix(svg, label, options, clear)
    }
    #[cfg(not(unix))]
    {
        let _ = (svg, label, clear);
        Ok(false)
    }
}

#[cfg(unix)]
fn show_svg_unix(svg: &str, label: &str, options: PreviewOptions, clear: bool) -> Result<bool> {
    use std::{
        fs::OpenOptions,
        io::Write,
        process::{Command, Stdio},
    };

    let format = match options.mode {
        TerminalPreviewMode::Auto => None,
        TerminalPreviewMode::Symbols => Some("symbols"),
        TerminalPreviewMode::Graphics => match detected_graphics_format() {
            Some(format) => Some(format),
            None => return Ok(false),
        },
        TerminalPreviewMode::Never => return Ok(false),
    };

    let png = rasterize(svg, options.width)?;
    let mut terminal = OpenOptions::new()
        .read(true)
        .write(true)
        .open("/dev/tty")
        .context("failed to open controlling terminal for preview")?;
    if clear {
        terminal
            .write_all(b"\x1b[2J\x1b[H")
            .context("failed to clear terminal preview")?;
    }
    writeln!(terminal, "Preview: {label}").context("failed to write terminal preview label")?;

    let mut command = Command::new("chafa");
    command.args([
        "--probe=auto",
        "--probe-mode=ctty",
        "--passthrough=auto",
        "--animate=off",
        "--bg=#f2f2f2",
        "--size",
        &format!("{}x", options.width),
    ]);
    if let Some(format) = format {
        command.args(["--format", format]);
    }
    let mut child = command
        .arg("-")
        .stdin(Stdio::piped())
        .stdout(Stdio::from(
            terminal
                .try_clone()
                .context("failed to clone controlling terminal")?,
        ))
        .stderr(Stdio::null())
        .spawn()
        .context("failed to start chafa; ensure it is installed or use --terminal-preview never")?;
    child
        .stdin
        .take()
        .context("failed to open chafa input")?
        .write_all(&png)
        .context("failed to send preview image to chafa")?;
    let status = child.wait().context("failed to wait for chafa")?;
    if !status.success() {
        bail!("chafa exited with status {status}");
    }
    Ok(true)
}

fn rasterize(svg: &str, width_cells: u16) -> Result<Vec<u8>> {
    let tree = usvg::Tree::from_str(svg, &usvg::Options::default())
        .context("failed to parse rendered SVG for terminal preview")?;
    let source = tree.size();
    let requested_width = (u32::from(width_cells) * 12).max(64);
    let mut scale = requested_width as f32 / source.width();
    let requested_height = (source.height() * scale).ceil();
    if requested_height > 1600.0 {
        scale *= 1600.0 / requested_height;
    }
    let width = (source.width() * scale).ceil().max(1.0) as u32;
    let height = (source.height() * scale).ceil().max(1.0) as u32;
    let mut pixmap = resvg::tiny_skia::Pixmap::new(width, height)
        .context("terminal preview dimensions are too large")?;
    pixmap.fill(resvg::tiny_skia::Color::from_rgba8(242, 242, 242, 255));
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::from_scale(scale, scale),
        &mut pixmap.as_mut(),
    );
    pixmap
        .encode_png()
        .context("failed to encode terminal preview PNG")
}

#[cfg(unix)]
fn detected_graphics_format() -> Option<&'static str> {
    let term = std::env::var("TERM")
        .unwrap_or_default()
        .to_ascii_lowercase();
    let program = std::env::var("TERM_PROGRAM")
        .unwrap_or_default()
        .to_ascii_lowercase();
    if program.contains("iterm") {
        Some("iterm")
    } else if term.contains("kitty")
        || term.contains("ghostty")
        || program.contains("wezterm")
        || program.contains("ghostty")
    {
        Some("kitty")
    } else if term.contains("sixel") || term.contains("foot") || term.contains("mlterm") {
        Some("sixels")
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rasterizes_path_only_svg() {
        let png = rasterize(
            r##"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="5"><path fill="#000" d="M0 0H10V5H0Z"/></svg>"##,
            20,
        )
        .unwrap();
        assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
    }
}
