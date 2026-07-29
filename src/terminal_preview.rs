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

/// Reuses terminal detection, the controlling terminal, and dimensions across
/// a batch of previews. This avoids spawning tmux detection and reopening the
/// terminal once per listed file.
pub struct PreviewSession {
    options: PreviewOptions,
    #[cfg(unix)]
    unix: Option<UnixPreviewSession>,
}

impl PreviewSession {
    pub fn new(options: PreviewOptions) -> Result<Self> {
        #[cfg(unix)]
        let unix = if options.enabled() {
            UnixPreviewSession::new(options)?
        } else {
            None
        };
        Ok(Self {
            options,
            #[cfg(unix)]
            unix,
        })
    }

    pub fn show_svg(&mut self, svg: &str, label: &str, clear: bool) -> Result<bool> {
        if !self.options.enabled() {
            return Ok(false);
        }
        let tree = usvg::Tree::from_str(svg, &usvg::Options::default())
            .context("failed to parse rendered SVG for terminal preview")?;
        self.show_tree(&tree, label, clear)
    }

    pub fn show_tree(&mut self, tree: &usvg::Tree, label: &str, clear: bool) -> Result<bool> {
        if !self.options.enabled() {
            return Ok(false);
        }
        #[cfg(unix)]
        {
            let Some(unix) = &mut self.unix else {
                return Ok(false);
            };
            unix.show_tree(tree, label, self.options, clear)?;
            Ok(true)
        }
        #[cfg(not(unix))]
        {
            let _ = (tree, label, clear);
            Ok(false)
        }
    }
}

pub fn show_svg(svg: &str, label: &str, options: PreviewOptions, clear: bool) -> Result<bool> {
    PreviewSession::new(options)?.show_svg(svg, label, clear)
}

#[cfg(unix)]
struct UnixPreviewSession {
    terminal: std::fs::File,
    encoder: RasterEncoder,
    dimensions: TerminalDimensions,
    wininfo: Wininfo,
}

#[cfg(unix)]
impl UnixPreviewSession {
    fn new(options: PreviewOptions) -> Result<Option<Self>> {
        use std::fs::OpenOptions;

        let environment = EnvIdentifiers::new();
        let Some(encoder) = select_encoder(options.mode, &environment) else {
            return Ok(None);
        };
        let terminal = OpenOptions::new()
            .read(true)
            .write(true)
            .open("/dev/tty")
            .context("failed to open controlling terminal for preview")?;
        let dimensions = terminal_dimensions(&terminal);
        let wininfo = dimensions.wininfo(&environment)?;
        Ok(Some(Self {
            terminal,
            encoder,
            dimensions,
            wininfo,
        }))
    }

    fn show_tree(
        &mut self,
        tree: &usvg::Tree,
        label: &str,
        options: PreviewOptions,
        clear: bool,
    ) -> Result<()> {
        use std::io::Write;

        let columns = options.width.min(self.dimensions.columns).max(1);
        let target_pixel_width = if self.encoder == RasterEncoder::Ascii {
            u32::from(columns)
        } else {
            u32::from(columns) * self.dimensions.cell_pixel_width()
        };
        let image = rasterize(tree, target_pixel_width)?;

        if clear {
            self.terminal
                .write_all(b"\x1b[2J\x1b[H")
                .context("failed to clear terminal preview")?;
        }
        writeln!(self.terminal, "Preview: {label}")
            .context("failed to write terminal preview label")?;

        let rows = preview_rows(&image, self.encoder, &self.wininfo)?;
        let rasteroid_handles_space = self.wininfo.is_tmux
            && matches!(self.encoder, RasterEncoder::Iterm | RasterEncoder::Sixel);
        if self.encoder != RasterEncoder::Ascii && !rasteroid_handles_space {
            ensure_space(&mut self.terminal, rows)
                .context("failed to reserve terminal preview rows")?;
        }

        self.encoder
            .encode_image(&image, &mut self.terminal, &self.wininfo, None, None)
            .with_context(|| format!("failed to write {:?} terminal preview", self.encoder))?;

        if self.encoder != RasterEncoder::Ascii {
            if !rasteroid_handles_space {
                write!(self.terminal, "\x1b[{rows}B")
                    .context("failed to advance past terminal preview")?;
            }
            writeln!(self.terminal).context("failed to finish terminal preview")?;
        }
        self.terminal
            .flush()
            .context("failed to flush terminal preview")?;
        Ok(())
    }
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

fn rasterize(tree: &usvg::Tree, requested_width: u32) -> Result<DynamicImage> {
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
        tree,
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
        let tree = usvg::Tree::from_str(
            r##"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="5"><path fill="#000" d="M0 0H10V5H0Z"/></svg>"##,
            &usvg::Options::default(),
        )
        .unwrap();
        let image = rasterize(&tree, 20).unwrap();
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
