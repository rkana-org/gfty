use std::path::Path;

use anyhow::{Context, Result, bail};
use xmltree::{Element, EmitterConfig, XMLNode};

use crate::{
    compose::RenderedLabel,
    export::{EXPORT_VERSION, ExportDocument, LabelInstance},
};

pub struct PlateOutput {
    pub svg: String,
    pub document: ExportDocument,
}

pub fn build_plate(
    label_paths: &[impl AsRef<Path>],
    dimensions: &[String],
    column_gap: &str,
    row_gap: &str,
    system_fonts: bool,
) -> Result<PlateOutput> {
    if label_paths.is_empty() {
        bail!("plate needs at least one label");
    }
    if dimensions.len() != 2 {
        bail!("plate dimensions require width and height");
    }
    let maximum_size = [
        crate::config::parse_length_mm(&dimensions[0]).context("invalid maximum plate width")?,
        crate::config::parse_length_mm(&dimensions[1]).context("invalid maximum plate height")?,
    ];
    if maximum_size[0] <= 0.0 || maximum_size[1] <= 0.0 {
        bail!("maximum plate dimensions must be positive");
    }
    let column_gap = crate::config::parse_length_mm(column_gap).context("invalid column gap")?;
    let row_gap = crate::config::parse_length_mm(row_gap).context("invalid row gap")?;

    let labels = label_paths
        .iter()
        .map(|path| {
            crate::config::LoadedLabel::load(path.as_ref())
                .with_context(|| format!("failed to load plate label {}", path.as_ref().display()))
        })
        .collect::<Result<Vec<_>>>()?;
    let mut filaments = std::collections::BTreeSet::new();
    for label in &labels {
        filaments.extend(
            crate::compose::label_filaments(label).with_context(|| {
                format!("failed to inspect filaments in {}", label.path.display())
            })?,
        );
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
                .with_context(|| format!("failed to render plate label {}", label.path.display()))
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

    let maximum_columns = cells_that_fit(maximum_size[0], cell_size[0], column_gap);
    let maximum_rows = cells_that_fit(maximum_size[1], cell_size[1], row_gap);
    if maximum_columns == 0 || maximum_rows == 0 {
        bail!(
            "label viewport {} x {} mm does not fit maximum plate dimensions {} x {} mm",
            cell_size[0],
            cell_size[1],
            maximum_size[0],
            maximum_size[1]
        );
    }
    let columns = maximum_columns.min(rendered.len());
    let required_rows = rendered.len().div_ceil(columns);
    if required_rows > maximum_rows {
        bail!(
            "{} labels need {} rows of {} columns, but only {} rows fit within {} x {} mm",
            rendered.len(),
            required_rows,
            columns,
            maximum_rows,
            maximum_size[0],
            maximum_size[1]
        );
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

fn cells_that_fit(maximum: f64, cell: f64, gap: f64) -> usize {
    if maximum < cell {
        0
    } else {
        ((maximum + gap) / (cell + gap)).floor() as usize
    }
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
    let mut filaments = std::collections::BTreeSet::new();
    let mut labels = Vec::<LabelInstance>::with_capacity(rendered.len());
    for (index, (label, center)) in rendered.iter().zip(&layout.centers).enumerate() {
        let document = crate::export::export_rendered(label)
            .with_context(|| format!("failed to export plate label at index {index}"))?;
        let [mut instance] = <[_; 1]>::try_from(document.labels).map_err(|_| {
            anyhow::anyhow!("individual label export did not contain exactly one label")
        })?;
        instance.center = *center;
        filaments.extend(document.filaments);
        labels.push(instance);
    }
    Ok(ExportDocument {
        version: EXPORT_VERSION,
        size: layout.size,
        filaments: filaments.into_iter().collect(),
        labels,
    })
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
        )
        .with_context(|| format!("failed to normalize plate preview label at index {index}"))?;
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

    #[test]
    fn computes_capacity_from_maximum_dimensions() {
        assert_eq!(cells_that_fit(200.0, 42.0, 5.0), 4);
        assert_eq!(cells_that_fit(42.0, 42.0, 5.0), 1);
        assert_eq!(cells_that_fit(41.9, 42.0, 5.0), 0);
    }

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
    fn combines_local_label_documents_with_centers_and_filaments() {
        let rendered = [
            RenderedLabel {
                svg: r##"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="5"><path fill="#000000" d="M0 0H10V5H0Z"/></svg>"##.to_owned(),
                palette: crate::color::PreviewPalette::new([0, 2]).unwrap(),
                size_mm: [10.0, 5.0],
            },
            RenderedLabel {
                svg: r##"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="5"><path fill="#00a000" d="M0 0H10V5H0Z"/></svg>"##.to_owned(),
                palette: crate::color::PreviewPalette::new([0, 2]).unwrap(),
                size_mm: [10.0, 5.0],
            },
        ];
        let layout = grid_layout(2, 2, [10.0, 5.0], 2.0, 1.0);
        let document = combine_documents(&rendered, &layout).unwrap();
        assert_eq!(document.version, 2);
        assert_eq!(document.size, [22.0, 5.0]);
        assert_eq!(document.filaments, [0, 2]);
        assert_eq!(document.labels.len(), 2);
        assert_eq!(document.labels[0].center, [-6.0, 0.0]);
        assert_eq!(document.labels[1].center, [6.0, 0.0]);
        assert_eq!(document.labels[0].parts[0].filament, 0);
        assert_eq!(document.labels[1].parts[0].filament, 2);
    }
}
