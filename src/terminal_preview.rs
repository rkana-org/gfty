use std::io::IsTerminal;

use anyhow::{Context, Result};
use image::{DynamicImage, RgbaImage};
use rasteroid::{
    Encoder, RasterEncoder,
    term_misc::{EnvIdentifiers, SizeDirection, Wininfo, ensure_space},
};

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
    use std::{fs::OpenOptions, io::Write};

    let environment = EnvIdentifiers::new();
    let Some(encoder) = select_encoder(options.mode, &environment) else {
        return Ok(false);
    };

    let mut terminal = OpenOptions::new()
        .read(true)
        .write(true)
        .open("/dev/tty")
        .context("failed to open controlling terminal for preview")?;
    let dimensions = terminal_dimensions(&terminal);
    let columns = options.width.min(dimensions.columns).max(1);
    let target_pixel_width = if encoder == RasterEncoder::Ascii {
        u32::from(columns)
    } else {
        u32::from(columns) * dimensions.cell_pixel_width()
    };
    let image = rasterize(svg, target_pixel_width)?;
    let wininfo = dimensions.wininfo(&environment)?;

    if clear {
        terminal
            .write_all(b"\x1b[2J\x1b[H")
            .context("failed to clear terminal preview")?;
    }
    writeln!(terminal, "Preview: {label}").context("failed to write terminal preview label")?;

    let rows = preview_rows(&image, encoder, &wininfo)?;
    let rasteroid_handles_space =
        wininfo.is_tmux && matches!(encoder, RasterEncoder::Iterm | RasterEncoder::Sixel);
    if encoder != RasterEncoder::Ascii && !rasteroid_handles_space {
        ensure_space(&mut terminal, rows).context("failed to reserve terminal preview rows")?;
    }

    encoder
        .encode_image(&image, &mut terminal, &wininfo, None, None)
        .with_context(|| format!("failed to write {encoder:?} terminal preview"))?;

    if encoder != RasterEncoder::Ascii {
        if !rasteroid_handles_space {
            write!(terminal, "\x1b[{rows}B").context("failed to advance past terminal preview")?;
        }
        writeln!(terminal).context("failed to finish terminal preview")?;
    }
    terminal
        .flush()
        .context("failed to flush terminal preview")?;
    Ok(true)
}

fn select_encoder(
    mode: TerminalPreviewMode,
    environment: &EnvIdentifiers,
) -> Option<RasterEncoder> {
    match mode {
        TerminalPreviewMode::Auto => Some(RasterEncoder::auto_detect(environment)),
        TerminalPreviewMode::Graphics => {
            let encoder = RasterEncoder::auto_detect(environment);
            (encoder != RasterEncoder::Ascii).then_some(encoder)
        }
        TerminalPreviewMode::Symbols => Some(RasterEncoder::Ascii),
        TerminalPreviewMode::Never => None,
    }
}

fn rasterize(svg: &str, requested_width: u32) -> Result<DynamicImage> {
    let tree = usvg::Tree::from_str(svg, &usvg::Options::default())
        .context("failed to parse rendered SVG for terminal preview")?;
    let source = tree.size();
    let mut scale = requested_width.max(1) as f32 / source.width();
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
    let image = RgbaImage::from_raw(width, height, pixmap.take())
        .context("failed to convert rendered terminal preview")?;
    Ok(DynamicImage::ImageRgba8(image))
}

fn preview_rows(image: &DynamicImage, encoder: RasterEncoder, wininfo: &Wininfo) -> Result<u16> {
    if encoder == RasterEncoder::Ascii {
        return Ok(u16::try_from(image.height().div_ceil(2)).unwrap_or(u16::MAX));
    }
    let rows = wininfo
        .dim_to_cells(&format!("{}px", image.height()), SizeDirection::Height)
        .context("failed to calculate terminal preview height")?;
    Ok(u16::try_from(rows.max(1)).unwrap_or(u16::MAX))
}

#[cfg(unix)]
#[derive(Debug, Clone, Copy)]
struct TerminalDimensions {
    columns: u16,
    rows: u16,
    pixel_width: u16,
    pixel_height: u16,
}

#[cfg(unix)]
impl TerminalDimensions {
    fn cell_pixel_width(self) -> u32 {
        if self.pixel_width > 0 && self.columns > 0 {
            (u32::from(self.pixel_width) / u32::from(self.columns)).max(1)
        } else {
            12
        }
    }

    fn wininfo(self, environment: &EnvIdentifiers) -> Result<Wininfo> {
        let pixel_width = if self.pixel_width > 0 {
            self.pixel_width
        } else {
            self.columns.saturating_mul(12)
        };
        let pixel_height = if self.pixel_height > 0 {
            self.pixel_height
        } else {
            self.rows.saturating_mul(24)
        };
        Wininfo::new(
            Some(&format!("{pixel_width}x{pixel_height}")),
            Some(&format!("{}x{}", self.columns, self.rows)),
            None,
            None,
            environment,
        )
        .context("failed to determine terminal dimensions for preview")
    }
}

#[cfg(unix)]
fn terminal_dimensions(terminal: &std::fs::File) -> TerminalDimensions {
    use std::os::fd::AsRawFd;

    let mut size = std::mem::MaybeUninit::<libc::winsize>::zeroed();
    // SAFETY: TIOCGWINSZ initializes the winsize value and only reads the valid tty descriptor.
    let result = unsafe { libc::ioctl(terminal.as_raw_fd(), libc::TIOCGWINSZ, size.as_mut_ptr()) };
    if result == 0 {
        // SAFETY: ioctl reported success, so the winsize value has been initialized.
        let size = unsafe { size.assume_init() };
        return TerminalDimensions {
            columns: size.ws_col.max(1),
            rows: size.ws_row.max(1),
            pixel_width: size.ws_xpixel,
            pixel_height: size.ws_ypixel,
        };
    }
    TerminalDimensions {
        columns: 80,
        rows: 24,
        pixel_width: 0,
        pixel_height: 0,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;

    fn environment(values: &[(&str, &str)]) -> EnvIdentifiers {
        EnvIdentifiers {
            data: values
                .iter()
                .map(|(key, value)| ((*key).to_owned(), (*value).to_owned()))
                .collect::<HashMap<_, _>>(),
        }
    }

    #[test]
    fn rasterizes_path_only_svg_to_an_image() {
        let image = rasterize(
            r##"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="5"><path fill="#000" d="M0 0H10V5H0Z"/></svg>"##,
            20,
        )
        .unwrap();
        assert_eq!((image.width(), image.height()), (20, 10));
        assert_eq!(image.to_rgba8().get_pixel(0, 0).0, [0, 0, 0, 255]);
    }

    #[test]
    fn maps_preview_modes_to_native_encoders() {
        let plain = environment(&[]);
        assert_eq!(
            select_encoder(TerminalPreviewMode::Auto, &plain),
            Some(RasterEncoder::Ascii)
        );
        assert_eq!(select_encoder(TerminalPreviewMode::Graphics, &plain), None);
        assert_eq!(
            select_encoder(TerminalPreviewMode::Symbols, &plain),
            Some(RasterEncoder::Ascii)
        );
        assert_eq!(select_encoder(TerminalPreviewMode::Never, &plain), None);

        let kitty = environment(&[("TERM", "xterm-kitty")]);
        assert_eq!(
            select_encoder(TerminalPreviewMode::Graphics, &kitty),
            Some(RasterEncoder::Kitty)
        );
    }
}
