use std::collections::BTreeMap;

use anyhow::{Context, Result, bail};
use serde::{Serialize, ser::SerializeStruct};
use usvg::{Node, Paint, tiny_skia_path::PathSegment};

use crate::{color::PreviewPalette, compose::RenderedLabel};

pub const EXPORT_VERSION: u32 = 2;

#[derive(Debug, Serialize)]
pub struct ExportDocument {
    pub version: u32,
    /// Overall rectangular layout size, including gaps between labels.
    pub size: [f64; 2],
    /// Numeric order is also the intended lexicographic part-name priority.
    pub filaments: Vec<u32>,
    pub labels: Vec<LabelInstance>,
}

#[derive(Debug, Serialize)]
pub struct LabelInstance {
    pub center: [f64; 2],
    pub size: [f64; 2],
    pub parts: Vec<Part>,
}

#[derive(Debug, Serialize)]
pub struct Part {
    pub filament: u32,
    pub shapes: Vec<Shape>,
}

#[derive(Debug)]
pub struct Shape {
    pub contours: Vec<Contour>,
}

impl Serialize for Shape {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut shape = serializer.serialize_struct("Shape", 1)?;
        shape.serialize_field("path", &format_path(&self.contours))?;
        shape.end()
    }
}

fn format_path(contours: &[Contour]) -> String {
    let mut tokens = Vec::new();
    for contour in contours {
        if contour.segments.is_empty() {
            continue;
        }
        tokens.push("M".to_owned());
        push_point(&mut tokens, contour.start);
        for segment in &contour.segments {
            match segment {
                Segment::Line { to } => {
                    tokens.push("L".to_owned());
                    push_point(&mut tokens, *to);
                }
                Segment::Cubic { c1, c2, to } => {
                    tokens.push("C".to_owned());
                    push_point(&mut tokens, *c1);
                    push_point(&mut tokens, *c2);
                    push_point(&mut tokens, *to);
                }
            }
        }
        tokens.push("Z".to_owned());
    }
    tokens.join(" ")
}

fn push_point(tokens: &mut Vec<String>, point: [f64; 2]) {
    tokens.push(format_number(point[0]));
    tokens.push(format_number(point[1]));
}

fn format_number(value: f64) -> String {
    if value.abs() < 0.0000005 {
        return "0".to_owned();
    }
    // Nanometer-scale coordinate precision is ample for label geometry and
    // keeps pasted FeatureScript JSON compact.
    let value = format!("{value:.6}");
    value.trim_end_matches('0').trim_end_matches('.').to_owned()
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

    if by_filament.is_empty() {
        bail!("rendered label contains no filled geometry to export");
    }
    let parts: Vec<_> = by_filament
        .into_iter()
        .map(|(filament, shapes)| Part { filament, shapes })
        .collect();
    let filaments = parts.iter().map(|part| part.filament).collect();
    Ok(ExportDocument {
        version: EXPORT_VERSION,
        size: rendered.size_mm,
        filaments,
        labels: vec![LabelInstance {
            center: [0.0, 0.0],
            size: rendered.size_mm,
            parts,
        }],
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
                // usvg has already resolved the complete SVG transform stack,
                // including viewports and all ancestor transforms. Let
                // tiny-skia apply that matrix before converting coordinates to
                // centered physical millimeters.
                let transformed = path
                    .data()
                    .clone()
                    .transform(path.abs_transform())
                    .context("failed to apply resolved SVG path transform")?;
                let contours = convert_path(transformed.segments(), canvas, size_mm);
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
                    start: map_point(point, canvas, size_mm),
                    closed: false,
                    segments: Vec::new(),
                });
            }
            PathSegment::LineTo(to) => {
                if let Some(contour) = &mut current_contour {
                    contour.segments.push(Segment::Line {
                        to: map_point(to, canvas, size_mm),
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
                        c1: map_point(c1, canvas, size_mm),
                        c2: map_point(c2, canvas, size_mm),
                        to: map_point(to, canvas, size_mm),
                    });
                }
                current = Some(to);
            }
            PathSegment::CubicTo(c1, c2, to) => {
                if let Some(contour) = &mut current_contour {
                    contour.segments.push(Segment::Cubic {
                        c1: map_point(c1, canvas, size_mm),
                        c2: map_point(c2, canvas, size_mm),
                        to: map_point(to, canvas, size_mm),
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

fn map_point(point: usvg::tiny_skia_path::Point, canvas: [f64; 2], size_mm: [f64; 2]) -> [f64; 2] {
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
            svg: r##"<svg xmlns="http://www.w3.org/2000/svg" width="96" height="48"><path fill="#8aaed6" d="M0 0 L96 0 L96 48 Z"/></svg>"##.to_owned(),
            palette: PreviewPalette::new([3]).unwrap(),
            size_mm: [25.4, 12.7],
        };
        let output = export_rendered(&rendered).unwrap();
        assert_eq!(output.version, 2);
        assert_eq!(output.filaments, [3]);
        assert_eq!(output.labels.len(), 1);
        assert_eq!(output.labels[0].parts[0].filament, 3);
        let contour = &output.labels[0].parts[0].shapes[0].contours[0];
        assert_eq!(contour.start, [-12.7, 6.35]);
        match contour.segments[0] {
            Segment::Line { to } => assert_eq!(to, [12.7, 6.35]),
            _ => panic!("expected line"),
        }
        assert_eq!(output.labels[0].center, [0.0, 0.0]);
        let json = serde_json::to_value(&output).unwrap();
        let shape = &json["labels"][0]["parts"][0]["shapes"][0];
        assert!(shape.get("contours").is_none());
        assert_eq!(shape["path"], "M -12.7 6.35 L 12.7 6.35 L 12.7 -6.35 Z");
    }

    #[test]
    fn applies_viewbox_and_nested_affine_transforms_via_usvg() {
        let rendered = RenderedLabel {
            svg: r##"<svg xmlns="http://www.w3.org/2000/svg" width="200" height="100" viewBox="0 0 100 50"><g transform="translate(20 10)"><g transform="rotate(90)"><g transform="scale(2 3)"><path fill="#8aaed6" d="M1 1 L3 1 L3 2 Z"/></g></g></g></svg>"##.to_owned(),
            palette: PreviewPalette::new([3]).unwrap(),
            size_mm: [20.0, 10.0],
        };

        let output = export_rendered(&rendered).unwrap();
        let contour = &output.labels[0].parts[0].shapes[0].contours[0];
        assert_point_close(contour.start, [-6.6, 2.6]);
        let Segment::Line { to } = contour.segments[0] else {
            panic!("expected line");
        };
        assert_point_close(to, [-6.6, 1.8]);
        let Segment::Line { to } = contour.segments[1] else {
            panic!("expected line");
        };
        assert_point_close(to, [-7.2, 1.8]);
    }

    fn assert_point_close(actual: [f64; 2], expected: [f64; 2]) {
        assert!((actual[0] - expected[0]).abs() < 1e-6, "{actual:?}");
        assert!((actual[1] - expected[1]).abs() < 1e-6, "{actual:?}");
    }
}
