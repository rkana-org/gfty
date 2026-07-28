use std::{collections::BTreeMap, path::Path};

use anyhow::{Context, Result, bail};
use xmltree::{Element, EmitterConfig, XMLNode};

use crate::{
    compose::RenderedLabel,
    export::{ExportDocument, Part, Segment, Shape},
};

pub struct PlateOutput {
    pub svg: String,
    pub document: ExportDocument,
}

pub fn build_plate(
    label_paths: &[impl AsRef<Path>],
    columns: usize,
    column_gap: &str,
    row_gap: &str,
    system_fonts: bool,
) -> Result<PlateOutput> {
    if label_paths.is_empty() {
        bail!("plate needs at least one label");
    }
    if columns == 0 {
        bail!("plate columns must be greater than zero");
    }
    let column_gap = crate::config::parse_length_mm(column_gap).context("invalid column gap")?;
    let row_gap = crate::config::parse_length_mm(row_gap).context("invalid row gap")?;

    let labels = label_paths
        .iter()
        .map(|path| crate::config::LoadedLabel::load(path.as_ref()))
        .collect::<Result<Vec<_>>>()?;
    let mut filaments = std::collections::BTreeSet::new();
    for label in &labels {
        filaments.extend(crate::compose::label_filaments(label)?);
    }
    let palette = crate::color::PreviewPalette::new(filaments)?;
    let project_roots = labels
        .iter()
        .map(|label| label.project_root.clone())
        .collect::<Vec<_>>();
    let rendered = labels
        .iter()
        .map(|label| {
            crate::compose::render_label_with_palette(label, system_fonts, palette.clone())
        })
        .collect::<Result<Vec<_>>>()?;

    let cell_size = rendered[0].size_mm;
    for (index, label) in rendered.iter().enumerate().skip(1) {
        if !same_size(label.size_mm, cell_size) {
            bail!(
                "label {} has viewport {} x {} mm, expected {} x {} mm",
                label_paths[index].as_ref().display(),
                label.size_mm[0],
                label.size_mm[1],
                cell_size[0],
                cell_size[1]
            );
        }
    }

    let layout = grid_layout(rendered.len(), columns, cell_size, column_gap, row_gap);
    let document = combine_documents(&rendered, &layout)?;
    let svg = combine_svgs(&rendered, &project_roots, &layout)?;
    Ok(PlateOutput { svg, document })
}

#[derive(Debug)]
struct GridLayout {
    size: [f64; 2],
    centers: Vec<[f64; 2]>,
    top_left: Vec<[f64; 2]>,
}

fn grid_layout(
    count: usize,
    columns: usize,
    cell: [f64; 2],
    column_gap: f64,
    row_gap: f64,
) -> GridLayout {
    let rows = count.div_ceil(columns);
    let width = columns as f64 * cell[0] + columns.saturating_sub(1) as f64 * column_gap;
    let height = rows as f64 * cell[1] + rows.saturating_sub(1) as f64 * row_gap;
    let mut centers = Vec::with_capacity(count);
    let mut top_left = Vec::with_capacity(count);

    for index in 0..count {
        let column = index % columns;
        let row = index / columns;
        let x_from_left = column as f64 * (cell[0] + column_gap);
        let y_from_top = row as f64 * (cell[1] + row_gap);
        top_left.push([x_from_left, y_from_top]);
        centers.push([
            -width / 2.0 + x_from_left + cell[0] / 2.0,
            height / 2.0 - y_from_top - cell[1] / 2.0,
        ]);
    }

    GridLayout {
        size: [width, height],
        centers,
        top_left,
    }
}

fn combine_documents(rendered: &[RenderedLabel], layout: &GridLayout) -> Result<ExportDocument> {
    let mut by_filament = BTreeMap::<u32, Vec<Shape>>::new();
    for (label, center) in rendered.iter().zip(&layout.centers) {
        let document = crate::export::export_rendered(label)?;
        for part in document.parts {
            let shapes = by_filament.entry(part.filament).or_default();
            for mut shape in part.shapes {
                translate_shape(&mut shape, *center);
                shapes.push(shape);
            }
        }
    }
    let parts = by_filament
        .into_iter()
        .map(|(filament, shapes)| Part { filament, shapes })
        .collect();
    Ok(ExportDocument {
        size: layout.size,
        parts,
        instances: layout.centers.clone(),
    })
}

fn translate_shape(shape: &mut Shape, offset: [f64; 2]) {
    for contour in &mut shape.contours {
        translate_point(&mut contour.start, offset);
        for segment in &mut contour.segments {
            match segment {
                Segment::Line { to } => translate_point(to, offset),
                Segment::Cubic { c1, c2, to } => {
                    translate_point(c1, offset);
                    translate_point(c2, offset);
                    translate_point(to, offset);
                }
            }
        }
    }
}

fn translate_point(point: &mut [f64; 2], offset: [f64; 2]) {
    point[0] += offset[0];
    point[1] += offset[1];
}

fn combine_svgs(
    rendered: &[RenderedLabel],
    project_roots: &[std::path::PathBuf],
    layout: &GridLayout,
) -> Result<String> {
    let mut root = Element::parse(br#"<svg xmlns="http://www.w3.org/2000/svg"></svg>"#.as_slice())
        .expect("static SVG root is valid");
    root.attributes
        .insert("width".to_owned(), format!("{}mm", layout.size[0]));
    root.attributes
        .insert("height".to_owned(), format!("{}mm", layout.size[1]));
    root.attributes.insert(
        "viewBox".to_owned(),
        format!("0 0 {} {}", layout.size[0], layout.size[1]),
    );

    for (index, ((label, project_root), position)) in rendered
        .iter()
        .zip(project_roots)
        .zip(&layout.top_left)
        .enumerate()
    {
        let prefixed = crate::svg::normalize_svg_with_prefix(
            &label.svg,
            project_root,
            project_root,
            false,
            Some(format!("plate-{index}-")),
        )?;
        let mut label_root =
            Element::parse(prefixed.as_bytes()).context("invalid rendered label SVG")?;
        let source_width = label_root
            .attributes
            .get("width")
            .context("rendered label SVG has no width")?;
        let source_height = label_root
            .attributes
            .get("height")
            .context("rendered label SVG has no height")?;
        let mut nested = Element::new("svg");
        nested.namespace = root.namespace.clone();
        nested
            .attributes
            .insert("x".to_owned(), position[0].to_string());
        nested
            .attributes
            .insert("y".to_owned(), position[1].to_string());
        nested
            .attributes
            .insert("width".to_owned(), label.size_mm[0].to_string());
        nested
            .attributes
            .insert("height".to_owned(), label.size_mm[1].to_string());
        nested.attributes.insert(
            "viewBox".to_owned(),
            format!("0 0 {source_width} {source_height}"),
        );
        nested.children = std::mem::take(&mut label_root.children);
        root.children.push(XMLNode::Element(nested));
    }

    let mut output = Vec::new();
    root.write_with_config(
        &mut output,
        EmitterConfig::new()
            .write_document_declaration(false)
            .perform_indent(false),
    )
    .context("failed to serialize plate SVG")?;
    let source = String::from_utf8(output).context("plate SVG is not UTF-8")?;
    crate::svg::normalize_svg(&source, &project_roots[0], &project_roots[0], false)
}

fn same_size(left: [f64; 2], right: [f64; 2]) -> bool {
    (left[0] - right[0]).abs() < 1e-9 && (left[1] - right[1]).abs() < 1e-9
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::export::{Contour, Segment};

    #[test]
    fn lays_out_fixed_columns_from_top_left() {
        let layout = grid_layout(4, 3, [10.0, 5.0], 2.0, 1.0);
        assert_eq!(layout.size, [34.0, 11.0]);
        assert_eq!(
            layout.centers,
            vec![[-12.0, 3.0], [0.0, 3.0], [12.0, 3.0], [-12.0, -3.0]]
        );
        assert_eq!(layout.top_left[3], [0.0, 6.0]);
    }

    #[test]
    fn translates_all_contour_points() {
        let mut shape = Shape {
            contours: vec![Contour {
                start: [0.0, 0.0],
                closed: true,
                segments: vec![Segment::Cubic {
                    c1: [1.0, 1.0],
                    c2: [2.0, 2.0],
                    to: [3.0, 3.0],
                }],
            }],
        };
        translate_shape(&mut shape, [10.0, -5.0]);
        assert_eq!(shape.contours[0].start, [10.0, -5.0]);
        match &shape.contours[0].segments[0] {
            Segment::Cubic { c1, c2, to } => {
                assert_eq!(*c1, [11.0, -4.0]);
                assert_eq!(*c2, [12.0, -3.0]);
                assert_eq!(*to, [13.0, -2.0]);
            }
            Segment::Line { .. } => panic!("expected cubic"),
        }
    }
}
