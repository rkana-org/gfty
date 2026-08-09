use std::collections::BTreeMap;

use anyhow::{Context, Result, bail};
use serde::{Serialize, ser::SerializeStruct};
use usvg::{
    Node, Paint,
    tiny_skia_path::{Path, PathSegment, PathStroker, Point},
};

use crate::{color::PreviewPalette, compose::RenderedLabel};

pub const EXPORT_VERSION: u32 = 2;

// Onshape accepts cubic Beziers but does not form a sketch region when an end
// control is exactly coincident with its adjacent endpoint. Move only such
// controls by at most one micron toward the next distinct control. The
// resulting geometric difference is far below print resolution while
// preserving closed SVG holes.
const ONSHAPE_CONTROL_NUDGE_FRACTION: f64 = 0.001;
const ONSHAPE_CONTROL_NUDGE_MAX_MM: f64 = 0.001;

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
    pub filament: u32,
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
        bail!("rendered label contains no fill or stroke geometry to export");
    }
    let parts: Vec<_> = by_filament
        .into_iter()
        .map(|(filament, shapes)| Part { filament, shapes })
        .collect();
    let mut filaments = parts.iter().map(|part| part.filament).collect::<Vec<_>>();
    filaments.push(rendered.base_filament);
    filaments.sort_unstable();
    filaments.dedup();
    Ok(ExportDocument {
        version: EXPORT_VERSION,
        size: rendered.size_mm,
        filaments,
        labels: vec![LabelInstance {
            center: [0.0, 0.0],
            size: rendered.size_mm,
            filament: rendered.base_filament,
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

                // A line has the SVG default black fill even though that fill
                // cannot cover an area. Avoid treating it as filament geometry;
                // in particular, source <line> elements do not discover a
                // spurious black filament before usvg converts them to paths.
                if let Some(fill) = path.fill()
                    && path_can_form_filled_region(path.data())
                {
                    let transformed = transform_path(path.data(), path.abs_transform(), "fill")?;
                    collect_painted_path(
                        fill.paint(),
                        &transformed,
                        palette,
                        canvas,
                        size_mm,
                        result,
                    )?;
                }

                if let Some(stroke) = path.stroke() {
                    // SVG strokes are defined in the path's local coordinate
                    // system and transformed with the path. Expand first so
                    // non-uniform transforms scale the outline correctly.
                    let outlined = expand_stroke(path, stroke)?;
                    let transformed = transform_path(&outlined, path.abs_transform(), "stroke")?;
                    collect_painted_path(
                        stroke.paint(),
                        &transformed,
                        palette,
                        canvas,
                        size_mm,
                        result,
                    )?;
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

fn transform_path(path: &Path, transform: usvg::Transform, paint: &str) -> Result<Path> {
    path.clone()
        .transform(transform)
        .with_context(|| format!("failed to apply resolved SVG {paint} transform"))
}

fn expand_stroke(path: &usvg::Path, stroke: &usvg::Stroke) -> Result<Path> {
    let mut style = stroke.to_tiny_skia();
    let resolution_scale = PathStroker::compute_resolution_scale(&path.abs_transform());

    // Path::stroke does not apply Stroke::dash itself. Match tiny-skia's
    // renderer by dashing the centerline before constructing its outline.
    let dashed = style
        .dash
        .take()
        .map(|dash| {
            path.data()
                .dash(&dash, resolution_scale)
                .context("failed to apply SVG stroke dash pattern")
        })
        .transpose()?;
    let centerline = dashed.as_ref().unwrap_or_else(|| path.data());
    centerline
        .stroke(&style, resolution_scale)
        .context("failed to expand SVG stroke into filled geometry")
}

fn collect_painted_path(
    paint: &Paint,
    path: &Path,
    palette: &PreviewPalette,
    canvas: [f64; 2],
    size_mm: [f64; 2],
    result: &mut BTreeMap<u32, Vec<Shape>>,
) -> Result<()> {
    let contours = convert_visible_path(path, canvas, size_mm);
    if contours.is_empty() {
        return Ok(());
    }

    let Paint::Color(color) = paint else {
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
    result.entry(filament).or_default().push(Shape { contours });
    Ok(())
}

fn path_can_form_filled_region(path: &Path) -> bool {
    let mut segment_count = 0usize;
    for segment in path.segments() {
        match segment {
            PathSegment::MoveTo(_) => segment_count = 0,
            PathSegment::LineTo(_) => {
                segment_count += 1;
                if segment_count >= 2 {
                    return true;
                }
            }
            // A single curved segment and its implicit closing line can
            // enclose an area, unlike a single straight segment.
            PathSegment::QuadTo(..) | PathSegment::CubicTo(..) => return true,
            PathSegment::Close => segment_count = 0,
        }
    }
    false
}

fn convert_visible_path(path: &Path, canvas: [f64; 2], size_mm: [f64; 2]) -> Vec<Contour> {
    if path_fits_canvas(path, canvas) {
        convert_path(path.segments(), canvas, size_mm)
    } else {
        // Export does not interpret SVG clip paths, so enforce the outer label
        // viewport here. Only paths crossing that boundary are flattened.
        convert_clipped_path(path.segments(), canvas, size_mm)
    }
}

fn path_fits_canvas(path: &Path, canvas: [f64; 2]) -> bool {
    let bounds = path.bounds();
    const TOLERANCE: f32 = 1e-5;
    bounds.left() >= -TOLERANCE
        && bounds.top() >= -TOLERANCE
        && bounds.right() <= canvas[0] as f32 + TOLERANCE
        && bounds.bottom() <= canvas[1] as f32 + TOLERANCE
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
                    contour.segments.push(onshape_cubic(
                        map_point(from, canvas, size_mm),
                        map_point(c1, canvas, size_mm),
                        map_point(c2, canvas, size_mm),
                        map_point(to, canvas, size_mm),
                    ));
                }
                current = Some(to);
            }
            PathSegment::CubicTo(c1, c2, to) => {
                if let (Some(from), Some(contour)) = (current, &mut current_contour) {
                    contour.segments.push(onshape_cubic(
                        map_point(from, canvas, size_mm),
                        map_point(c1, canvas, size_mm),
                        map_point(c2, canvas, size_mm),
                        map_point(to, canvas, size_mm),
                    ));
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

fn convert_clipped_path(
    segments: impl Iterator<Item = PathSegment>,
    canvas: [f64; 2],
    size_mm: [f64; 2],
) -> Vec<Contour> {
    const CURVE_STEPS: usize = 24;

    let mut contours = Vec::new();
    let mut current_polygon: Option<Vec<Point>> = None;
    let mut current = None;
    let mut subpath_start = None;

    let finish = |polygon: &mut Option<Vec<Point>>, result: &mut Vec<Contour>| {
        let Some(mut polygon) = polygon.take() else {
            return;
        };
        remove_duplicate_endpoint(&mut polygon);
        if polygon.len() < 3 {
            return;
        }
        let clipped = clip_polygon_to_canvas(polygon, canvas);
        if clipped.len() >= 3 {
            result.push(polygon_to_contour(&clipped, canvas, size_mm));
        }
    };

    for segment in segments {
        match segment {
            PathSegment::MoveTo(point) => {
                finish(&mut current_polygon, &mut contours);
                current_polygon = Some(vec![point]);
                current = Some(point);
                subpath_start = Some(point);
            }
            PathSegment::LineTo(to) => {
                if let Some(polygon) = &mut current_polygon {
                    push_distinct_point(polygon, to);
                }
                current = Some(to);
            }
            PathSegment::QuadTo(control, to) => {
                if let (Some(from), Some(polygon)) = (current, &mut current_polygon) {
                    for step in 1..=CURVE_STEPS {
                        let t = step as f32 / CURVE_STEPS as f32;
                        push_distinct_point(polygon, eval_quad(from, control, to, t));
                    }
                }
                current = Some(to);
            }
            PathSegment::CubicTo(c1, c2, to) => {
                if let (Some(from), Some(polygon)) = (current, &mut current_polygon) {
                    for step in 1..=CURVE_STEPS {
                        let t = step as f32 / CURVE_STEPS as f32;
                        push_distinct_point(polygon, eval_cubic(from, c1, c2, to, t));
                    }
                }
                current = Some(to);
            }
            PathSegment::Close => {
                finish(&mut current_polygon, &mut contours);
                current = subpath_start;
            }
        }
    }
    finish(&mut current_polygon, &mut contours);
    contours
}

fn remove_duplicate_endpoint(points: &mut Vec<Point>) {
    if points.len() >= 2 && points_close_canvas(points[0], *points.last().expect("non-empty")) {
        points.pop();
    }
}

fn push_distinct_point(points: &mut Vec<Point>, point: Point) {
    if points
        .last()
        .is_none_or(|previous| !points_close_canvas(*previous, point))
    {
        points.push(point);
    }
}

fn points_close_canvas(first: Point, second: Point) -> bool {
    (first.x - second.x).abs() < 1e-5 && (first.y - second.y).abs() < 1e-5
}

fn eval_quad(from: Point, control: Point, to: Point, t: f32) -> Point {
    let mt = 1.0 - t;
    Point::from_xy(
        mt * mt * from.x + 2.0 * mt * t * control.x + t * t * to.x,
        mt * mt * from.y + 2.0 * mt * t * control.y + t * t * to.y,
    )
}

fn eval_cubic(from: Point, c1: Point, c2: Point, to: Point, t: f32) -> Point {
    let mt = 1.0 - t;
    Point::from_xy(
        mt * mt * mt * from.x
            + 3.0 * mt * mt * t * c1.x
            + 3.0 * mt * t * t * c2.x
            + t * t * t * to.x,
        mt * mt * mt * from.y
            + 3.0 * mt * mt * t * c1.y
            + 3.0 * mt * t * t * c2.y
            + t * t * t * to.y,
    )
}

#[derive(Debug, Clone, Copy)]
enum ClipEdge {
    Left,
    Right,
    Top,
    Bottom,
}

fn clip_polygon_to_canvas(mut polygon: Vec<Point>, canvas: [f64; 2]) -> Vec<Point> {
    for edge in [
        ClipEdge::Left,
        ClipEdge::Right,
        ClipEdge::Top,
        ClipEdge::Bottom,
    ] {
        polygon = clip_polygon_edge(&polygon, edge, canvas);
        if polygon.is_empty() {
            break;
        }
    }
    remove_duplicate_endpoint(&mut polygon);
    polygon
}

fn clip_polygon_edge(polygon: &[Point], edge: ClipEdge, canvas: [f64; 2]) -> Vec<Point> {
    let mut result = Vec::new();
    let Some(&last) = polygon.last() else {
        return result;
    };
    let mut previous = last;
    let mut previous_inside = point_inside_edge(previous, edge, canvas);

    for &current in polygon {
        let current_inside = point_inside_edge(current, edge, canvas);
        if current_inside != previous_inside {
            push_distinct_point(&mut result, intersect_edge(previous, current, edge, canvas));
        }
        if current_inside {
            push_distinct_point(&mut result, current);
        }
        previous = current;
        previous_inside = current_inside;
    }
    result
}

fn point_inside_edge(point: Point, edge: ClipEdge, canvas: [f64; 2]) -> bool {
    const TOLERANCE: f32 = 1e-5;
    match edge {
        ClipEdge::Left => point.x >= -TOLERANCE,
        ClipEdge::Right => point.x <= canvas[0] as f32 + TOLERANCE,
        ClipEdge::Top => point.y >= -TOLERANCE,
        ClipEdge::Bottom => point.y <= canvas[1] as f32 + TOLERANCE,
    }
}

fn intersect_edge(from: Point, to: Point, edge: ClipEdge, canvas: [f64; 2]) -> Point {
    let (boundary, vertical) = match edge {
        ClipEdge::Left => (0.0, true),
        ClipEdge::Right => (canvas[0] as f32, true),
        ClipEdge::Top => (0.0, false),
        ClipEdge::Bottom => (canvas[1] as f32, false),
    };
    let denominator = if vertical {
        to.x - from.x
    } else {
        to.y - from.y
    };
    if denominator.abs() < 1e-12 {
        return to;
    }
    let t = if vertical {
        (boundary - from.x) / denominator
    } else {
        (boundary - from.y) / denominator
    };
    Point::from_xy(from.x + (to.x - from.x) * t, from.y + (to.y - from.y) * t)
}

fn polygon_to_contour(points: &[Point], canvas: [f64; 2], size_mm: [f64; 2]) -> Contour {
    let start = map_point(points[0], canvas, size_mm);
    let segments = points[1..]
        .iter()
        .map(|point| Segment::Line {
            to: map_point(*point, canvas, size_mm),
        })
        .collect();
    Contour {
        start,
        closed: true,
        segments,
    }
}

fn onshape_cubic(from: [f64; 2], mut c1: [f64; 2], mut c2: [f64; 2], to: [f64; 2]) -> Segment {
    let original_c1 = c1;
    let original_c2 = c2;
    if points_close(c1, from) {
        c1 = interpolate(
            from,
            if points_close(original_c2, from) {
                to
            } else {
                original_c2
            },
        );
    }
    if points_close(c2, to) {
        c2 = interpolate(
            to,
            if points_close(original_c1, to) {
                from
            } else {
                original_c1
            },
        );
    }
    Segment::Cubic { c1, c2, to }
}

fn points_close(first: [f64; 2], second: [f64; 2]) -> bool {
    (first[0] - second[0]).abs() < 1e-9 && (first[1] - second[1]).abs() < 1e-9
}

fn interpolate(from: [f64; 2], toward: [f64; 2]) -> [f64; 2] {
    let delta = [toward[0] - from[0], toward[1] - from[1]];
    let distance = delta[0].hypot(delta[1]);
    if distance == 0.0 {
        return from;
    }
    let fraction = ONSHAPE_CONTROL_NUDGE_FRACTION.min(ONSHAPE_CONTROL_NUDGE_MAX_MM / distance);
    [from[0] + delta[0] * fraction, from[1] + delta[1] * fraction]
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
            base_filament: 3,
        };
        let output = export_rendered(&rendered).unwrap();
        assert_eq!(output.version, 2);
        assert_eq!(output.filaments, [3]);
        assert_eq!(output.labels.len(), 1);
        assert_eq!(output.labels[0].filament, 3);
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
    fn exports_stroke_only_lines_as_closed_filled_geometry() {
        let rendered = RenderedLabel {
            // SVG gives line elements an implicit black fill, but that fill has
            // no area and is intentionally absent from this palette.
            svg: r##"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10"><line x1="2" y1="5" x2="8" y2="5" stroke="#8aaed6" stroke-width="2"/></svg>"##.to_owned(),
            palette: PreviewPalette::new([3]).unwrap(),
            size_mm: [10.0, 10.0],
            base_filament: 0,
        };

        let output = export_rendered(&rendered).unwrap();
        assert_eq!(output.filaments, [0, 3]);
        assert_eq!(output.labels[0].parts.len(), 1);
        let part = &output.labels[0].parts[0];
        assert_eq!(part.filament, 3);
        assert_eq!(part.shapes.len(), 1);
        assert_eq!(part.shapes[0].contours.len(), 1);
        assert_bounds_close(
            shape_control_bounds(&part.shapes[0]),
            [-3.0, -1.0, 3.0, 1.0],
        );

        let json = serde_json::to_value(&output).unwrap();
        let path = json["labels"][0]["parts"][0]["shapes"][0]["path"]
            .as_str()
            .unwrap();
        assert!(path.starts_with("M "), "{path}");
        assert!(path.ends_with(" Z"), "{path}");
    }

    #[test]
    fn exports_fill_and_stroke_as_separate_filament_geometry() {
        let rendered = RenderedLabel {
            svg: r##"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10"><path fill="#a7d293" stroke="#8aaed6" stroke-width="2" d="M3 3L7 3L7 7L3 7Z"/></svg>"##.to_owned(),
            palette: PreviewPalette::new([2, 3]).unwrap(),
            size_mm: [10.0, 10.0],
            base_filament: 0,
        };

        let output = export_rendered(&rendered).unwrap();
        assert_eq!(output.filaments, [0, 2, 3]);
        assert_eq!(output.labels[0].parts.len(), 2);
        let fill = &output.labels[0].parts[0];
        assert_eq!(fill.filament, 2);
        assert_bounds_close(
            shape_control_bounds(&fill.shapes[0]),
            [-2.0, -2.0, 2.0, 2.0],
        );
        let stroke = &output.labels[0].parts[1];
        assert_eq!(stroke.filament, 3);
        assert_eq!(stroke.shapes[0].contours.len(), 2);
        assert_bounds_close(
            shape_control_bounds(&stroke.shapes[0]),
            [-3.0, -3.0, 3.0, 3.0],
        );
    }

    #[test]
    fn expands_dashes_before_stroking() {
        let rendered = RenderedLabel {
            svg: r##"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10"><path fill="none" stroke="#8aaed6" stroke-width="1" stroke-dasharray="2 2" d="M1 5L9 5"/></svg>"##.to_owned(),
            palette: PreviewPalette::new([3]).unwrap(),
            size_mm: [10.0, 10.0],
            base_filament: 0,
        };

        let output = export_rendered(&rendered).unwrap();
        let contours = &output.labels[0].parts[0].shapes[0].contours;
        assert_eq!(contours.len(), 2);
        for contour in contours {
            assert!(contour.closed);
            assert!(!contour.segments.is_empty());
        }
    }

    #[test]
    fn expands_strokes_before_applying_non_uniform_transforms() {
        let rendered = RenderedLabel {
            svg: r##"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="20"><path transform="scale(2 3)" fill="none" stroke="#8aaed6" stroke-width="2" d="M2 2L4 2"/></svg>"##.to_owned(),
            palette: PreviewPalette::new([3]).unwrap(),
            size_mm: [20.0, 20.0],
            base_filament: 0,
        };

        let output = export_rendered(&rendered).unwrap();
        assert_bounds_close(
            shape_control_bounds(&output.labels[0].parts[0].shapes[0]),
            [-6.0, 1.0, -2.0, 7.0],
        );
    }

    #[test]
    fn nudges_endpoint_coincident_bezier_controls_for_onshape_regions() {
        let rendered = RenderedLabel {
            svg: r##"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10"><path fill="#8aaed6" d="M0 0 C0 0 10 5 10 5 L10 10 L0 10 Z"/></svg>"##.to_owned(),
            palette: PreviewPalette::new([3]).unwrap(),
            size_mm: [10.0, 10.0],
            base_filament: 3,
        };

        let output = export_rendered(&rendered).unwrap();
        let contour = &output.labels[0].parts[0].shapes[0].contours[0];
        let Segment::Cubic { c1, c2, to } = contour.segments[0] else {
            panic!("expected cubic");
        };
        assert_point_close(to, [5.0, 0.0]);
        assert_point_close(c1, [-4.999_105_573, 4.999_552_786]);
        assert_point_close(c2, [4.999_105_573, 0.000_447_214]);
        assert!(!points_close(c1, contour.start));
        assert!(!points_close(c2, to));
    }

    #[test]
    fn applies_viewbox_and_nested_affine_transforms_via_usvg() {
        let rendered = RenderedLabel {
            svg: r##"<svg xmlns="http://www.w3.org/2000/svg" width="200" height="100" viewBox="0 0 100 50"><g transform="translate(20 10)"><g transform="rotate(90)"><g transform="scale(2 3)"><path fill="#8aaed6" d="M1 1 L3 1 L3 2 Z"/></g></g></g></svg>"##.to_owned(),
            palette: PreviewPalette::new([3]).unwrap(),
            size_mm: [20.0, 10.0],
            base_filament: 3,
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

    fn shape_control_bounds(shape: &Shape) -> [f64; 4] {
        let mut bounds = [
            f64::INFINITY,
            f64::INFINITY,
            f64::NEG_INFINITY,
            f64::NEG_INFINITY,
        ];
        let mut include = |point: [f64; 2]| {
            bounds[0] = bounds[0].min(point[0]);
            bounds[1] = bounds[1].min(point[1]);
            bounds[2] = bounds[2].max(point[0]);
            bounds[3] = bounds[3].max(point[1]);
        };
        for contour in &shape.contours {
            include(contour.start);
            for segment in &contour.segments {
                match segment {
                    Segment::Line { to } => include(*to),
                    Segment::Cubic { c1, c2, to } => {
                        include(*c1);
                        include(*c2);
                        include(*to);
                    }
                }
            }
        }
        bounds
    }

    fn assert_bounds_close(actual: [f64; 4], expected: [f64; 4]) {
        for (actual, expected) in actual.into_iter().zip(expected) {
            assert!((actual - expected).abs() < 1e-5, "{actual} != {expected}");
        }
    }

    fn assert_point_close(actual: [f64; 2], expected: [f64; 2]) {
        assert!((actual[0] - expected[0]).abs() < 1e-6, "{actual:?}");
        assert!((actual[1] - expected[1]).abs() < 1e-6, "{actual:?}");
    }
}
