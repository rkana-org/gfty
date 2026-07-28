use std::collections::BTreeMap;

use anyhow::{Context, Result, bail};
use serde::Serialize;
use usvg::{Node, Paint, tiny_skia_path::PathSegment};

use crate::{color::PreviewPalette, compose::RenderedLabel};

#[derive(Debug, Serialize)]
pub struct ExportDocument {
    pub size: [f64; 2],
    pub parts: Vec<Part>,
    pub instances: Vec<[f64; 2]>,
}

#[derive(Debug, Serialize)]
pub struct Part {
    pub filament: u32,
    pub shapes: Vec<Shape>,
}

#[derive(Debug, Serialize)]
pub struct Shape {
    pub contours: Vec<Contour>,
}

#[derive(Debug, Serialize)]
pub struct Contour {
    pub start: [f64; 2],
    pub closed: bool,
    pub segments: Vec<Segment>,
}

#[derive(Debug, Serialize)]
#[serde(tag = "type")]
pub enum Segment {
    #[serde(rename = "L")]
    Line { to: [f64; 2] },
    #[serde(rename = "C")]
    Cubic {
        c1: [f64; 2],
        c2: [f64; 2],
        to: [f64; 2],
    },
}

pub fn export_rendered(rendered: &RenderedLabel) -> Result<ExportDocument> {
    let tree = usvg::Tree::from_str(&rendered.svg, &usvg::Options::default())
        .context("failed to parse rendered SVG for export")?;
    let canvas = [tree.size().width() as f64, tree.size().height() as f64];
    let mut by_filament = BTreeMap::<u32, Vec<Shape>>::new();
    collect_group(
        tree.root(),
        &rendered.palette,
        canvas,
        rendered.size_mm,
        &mut by_filament,
    )?;

    let parts = by_filament
        .into_iter()
        .map(|(filament, shapes)| Part { filament, shapes })
        .collect();
    Ok(ExportDocument {
        size: rendered.size_mm,
        parts,
        instances: vec![[0.0, 0.0]],
    })
}

fn collect_group(
    group: &usvg::Group,
    palette: &PreviewPalette,
    canvas: [f64; 2],
    size_mm: [f64; 2],
    result: &mut BTreeMap<u32, Vec<Shape>>,
) -> Result<()> {
    for node in group.children() {
        match node {
            Node::Group(child) => collect_group(child, palette, canvas, size_mm, result)?,
            Node::Path(path) => {
                if !path.is_visible() {
                    continue;
                }
                let Some(fill) = path.fill() else {
                    continue;
                };
                let Paint::Color(color) = fill.paint() else {
                    bail!("gradients and patterns cannot be exported as filament geometry");
                };
                let filament = palette
                    .filament(color.red, color.green, color.blue)
                    .with_context(|| {
                        format!(
                            "rendered color #{:02x}{:02x}{:02x} has no filament mapping",
                            color.red, color.green, color.blue
                        )
                    })?;
                let contours = convert_path(
                    path.data().segments(),
                    path.abs_transform(),
                    canvas,
                    size_mm,
                );
                if !contours.is_empty() {
                    result.entry(filament).or_default().push(Shape { contours });
                }
            }
            Node::Image(_) => bail!("raster images cannot be exported as filament geometry"),
            Node::Text(_) => {
                bail!("text remained after outlining; check that its font is available")
            }
        }
    }
    Ok(())
}

fn convert_path(
    segments: impl Iterator<Item = PathSegment>,
    transform: usvg::Transform,
    canvas: [f64; 2],
    size_mm: [f64; 2],
) -> Vec<Contour> {
    let mut contours = Vec::new();
    let mut current_contour: Option<Contour> = None;
    let mut current = None;
    let mut subpath_start = None;

    let finish = |contour: &mut Option<Contour>, result: &mut Vec<Contour>| {
        if let Some(mut contour) = contour.take()
            && !contour.segments.is_empty()
        {
            // SVG closes every subpath for filling even when `Z` is omitted.
            contour.closed = true;
            result.push(contour);
        }
    };

    for segment in segments {
        match segment {
            PathSegment::MoveTo(point) => {
                finish(&mut current_contour, &mut contours);
                current = Some(point);
                subpath_start = Some(point);
                current_contour = Some(Contour {
                    start: map_point(point, transform, canvas, size_mm),
                    closed: false,
                    segments: Vec::new(),
                });
            }
            PathSegment::LineTo(to) => {
                if let Some(contour) = &mut current_contour {
                    contour.segments.push(Segment::Line {
                        to: map_point(to, transform, canvas, size_mm),
                    });
                }
                current = Some(to);
            }
            PathSegment::QuadTo(control, to) => {
                if let (Some(from), Some(contour)) = (current, &mut current_contour) {
                    let c1 = usvg::tiny_skia_path::Point::from_xy(
                        from.x + (control.x - from.x) * 2.0 / 3.0,
                        from.y + (control.y - from.y) * 2.0 / 3.0,
                    );
                    let c2 = usvg::tiny_skia_path::Point::from_xy(
                        to.x + (control.x - to.x) * 2.0 / 3.0,
                        to.y + (control.y - to.y) * 2.0 / 3.0,
                    );
                    contour.segments.push(Segment::Cubic {
                        c1: map_point(c1, transform, canvas, size_mm),
                        c2: map_point(c2, transform, canvas, size_mm),
                        to: map_point(to, transform, canvas, size_mm),
                    });
                }
                current = Some(to);
            }
            PathSegment::CubicTo(c1, c2, to) => {
                if let Some(contour) = &mut current_contour {
                    contour.segments.push(Segment::Cubic {
                        c1: map_point(c1, transform, canvas, size_mm),
                        c2: map_point(c2, transform, canvas, size_mm),
                        to: map_point(to, transform, canvas, size_mm),
                    });
                }
                current = Some(to);
            }
            PathSegment::Close => {
                if let Some(contour) = &mut current_contour {
                    contour.closed = true;
                }
                current = subpath_start;
            }
        }
    }
    finish(&mut current_contour, &mut contours);
    contours
}

fn map_point(
    mut point: usvg::tiny_skia_path::Point,
    transform: usvg::Transform,
    canvas: [f64; 2],
    size_mm: [f64; 2],
) -> [f64; 2] {
    transform.map_point(&mut point);
    let x = point.x as f64 / canvas[0] * size_mm[0] - size_mm[0] / 2.0;
    let y = size_mm[1] / 2.0 - point.y as f64 / canvas[1] * size_mm[1];
    [clean_zero(x), clean_zero(y)]
}

fn clean_zero(value: f64) -> f64 {
    if value.abs() < 1e-12 { 0.0 } else { value }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exports_centered_mm_contours_by_filament() {
        let rendered = RenderedLabel {
            svg: r##"<svg xmlns="http://www.w3.org/2000/svg" width="96" height="48"><path fill="#ff0000" d="M0 0 L96 0 L96 48 Z"/></svg>"##.to_owned(),
            palette: PreviewPalette::new([3]).unwrap(),
            size_mm: [25.4, 12.7],
        };
        let output = export_rendered(&rendered).unwrap();
        assert_eq!(output.parts.len(), 1);
        assert_eq!(output.parts[0].filament, 3);
        let contour = &output.parts[0].shapes[0].contours[0];
        assert_eq!(contour.start, [-12.7, 6.35]);
        match contour.segments[0] {
            Segment::Line { to } => assert_eq!(to, [12.7, 6.35]),
            _ => panic!("expected line"),
        }
        assert_eq!(output.instances, vec![[0.0, 0.0]]);
    }
}
