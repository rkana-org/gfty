use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};

use crate::config::{ConfigKind, parse_length_mm};

pub const BIN_CONFIG_VERSION: u32 = 1;
pub const BIN_BODY_CONFIG_VERSION: u32 = 2;
pub const DEFAULT_BIN_MODEL_URL: &str = "https://cad.onshape.com/documents/044aa38d921c6673acd89aef/v/793cbd4a9bdd57cb44baa08a/e/47f09ccd9b344504691f98d4";
const GRIDFINITY_MM: f64 = 42.0;

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct BinConfig {
    pub kind: ConfigKind,
    pub version: u32,
    pub size: [u32; 3],

    #[serde(default)]
    pub base: BaseConfig,
    #[serde(default)]
    pub bin: BinBodyConfig,
    #[serde(default)]
    pub label: BinLabelConfig,
    #[serde(default)]
    pub divider: DividerConfig,
    #[serde(default)]
    pub easy_grab: EasyGrabConfig,
    #[serde(default)]
    pub print: PrintConfig,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields, rename_all = "kebab-case")]
pub struct BaseConfig {
    pub enabled: bool,
    pub rounded_corners: bool,
    pub magnets: bool,
    pub connector_cutouts: bool,
    pub connector_pin: bool,
}

impl Default for BaseConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            rounded_corners: false,
            magnets: true,
            connector_cutouts: true,
            connector_pin: true,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields, rename_all = "kebab-case")]
pub struct BinBodyConfig {
    pub enabled: bool,
    pub nesting: bool,
    pub swappable_rim: bool,
    pub spring_compensation: bool,
    pub additional_rim_expansion: String,
    pub tub: bool,
}

impl Default for BinBodyConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            nesting: true,
            swappable_rim: true,
            spring_compensation: true,
            additional_rim_expansion: "0mm".to_owned(),
            tub: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum SupportsMode {
    Always,
    Auto,
    Off,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields, rename_all = "kebab-case")]
pub struct BinLabelConfig {
    pub enabled: bool,
    pub depth: String,
    pub swappable: bool,
    pub supports: SupportsMode,
    pub embossing_clearance: String,
    pub embossing_inset: String,
    pub full_width: bool,
    pub width_units: f64,
}

impl Default for BinLabelConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            depth: "10mm".to_owned(),
            swappable: true,
            supports: SupportsMode::Auto,
            embossing_clearance: "0.4mm".to_owned(),
            embossing_inset: "0mm".to_owned(),
            full_width: true,
            width_units: 1.0,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields, rename_all = "kebab-case")]
pub struct DividerConfig {
    pub columns: Vec<String>,
    pub rows: Vec<String>,
    pub merges: Vec<DividerMerge>,
}

impl Default for DividerConfig {
    fn default() -> Self {
        Self {
            columns: vec!["auto".to_owned(); 3],
            rows: vec!["auto".to_owned(); 2],
            merges: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct DividerMerge {
    pub columns: [usize; 2],
    pub rows: [usize; 2],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum EasyGrabMode {
    None,
    Custom,
    All,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Side {
    North,
    South,
    East,
    West,
}

impl Side {
    fn as_str(self) -> &'static str {
        match self {
            Self::North => "north",
            Self::South => "south",
            Self::East => "east",
            Self::West => "west",
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields, rename_all = "kebab-case")]
pub struct EasyGrabConfig {
    pub mode: EasyGrabMode,
    pub side: Side,
    pub radius: String,
    pub faces: Vec<EasyGrabFace>,
}

impl Default for EasyGrabConfig {
    fn default() -> Self {
        Self {
            mode: EasyGrabMode::All,
            side: Side::South,
            radius: "21mm".to_owned(),
            faces: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct EasyGrabFace {
    pub side: Side,
    pub columns: [usize; 2],
    pub rows: [usize; 2],
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub radius: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields, rename_all = "kebab-case")]
pub struct PrintConfig {
    pub max_overhang: f64,
}

impl Default for PrintConfig {
    fn default() -> Self {
        Self { max_overhang: 60.0 }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum BinComponent {
    All,
    Bin,
    Base,
    SwappableRim,
    SwappableLabel,
    ConnectorPin,
}

impl BinComponent {
    pub fn part_name(self) -> Option<&'static str> {
        match self {
            Self::All => None,
            Self::Bin => Some("Bin"),
            Self::Base => Some("Base"),
            Self::SwappableRim => Some("SwappableRim"),
            Self::SwappableLabel => Some("SwappableLabel"),
            Self::ConnectorPin => Some("ConnectorPin"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum InterfaceMode {
    Off,
    Integrated,
    Swappable,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields, rename_all = "kebab-case")]
pub struct RimInterfaceConfig {
    pub mode: InterfaceMode,
}

impl Default for RimInterfaceConfig {
    fn default() -> Self {
        Self {
            mode: InterfaceMode::Swappable,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields, rename_all = "kebab-case")]
pub struct LabelInterfaceConfig {
    pub mode: InterfaceMode,
    pub depth: String,
    pub supports: SupportsMode,
}

impl Default for LabelInterfaceConfig {
    fn default() -> Self {
        Self {
            mode: InterfaceMode::Swappable,
            depth: "10mm".to_owned(),
            supports: SupportsMode::Auto,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct BinBodyFileConfig {
    pub kind: ConfigKind,
    pub version: u32,
    pub size: [u32; 3],
    #[serde(default = "default_true")]
    pub tub: bool,
    #[serde(default = "default_max_overhang")]
    pub max_print_overhang: f64,
    #[serde(default)]
    pub rim_interface: RimInterfaceConfig,
    #[serde(default)]
    pub label_interface: LabelInterfaceConfig,
    #[serde(default)]
    pub divider: DividerConfig,
    #[serde(default)]
    pub easy_grab: EasyGrabConfig,
}

fn default_true() -> bool {
    true
}

fn default_max_overhang() -> f64 {
    60.0
}

impl BinBodyFileConfig {
    fn into_carrier(self) -> Result<BinConfig> {
        if self.kind != ConfigKind::Bin {
            bail!("bin TOML kind must be \"bin\"");
        }
        if self.version != BIN_BODY_CONFIG_VERSION {
            bail!(
                "unsupported constituent bin TOML version {}; expected {BIN_BODY_CONFIG_VERSION}",
                self.version
            );
        }
        let mut carrier = BinConfig {
            kind: ConfigKind::Bin,
            version: BIN_CONFIG_VERSION,
            size: self.size,
            base: BaseConfig::default(),
            bin: BinBodyConfig::default(),
            label: BinLabelConfig::default(),
            divider: self.divider,
            easy_grab: self.easy_grab,
            print: PrintConfig {
                max_overhang: self.max_print_overhang,
            },
        };
        carrier.base.enabled = false;
        carrier.bin.tub = self.tub;
        carrier.bin.nesting = self.rim_interface.mode != InterfaceMode::Off;
        carrier.bin.swappable_rim = self.rim_interface.mode == InterfaceMode::Swappable;
        carrier.label.enabled = self.label_interface.mode != InterfaceMode::Off;
        carrier.label.swappable = self.label_interface.mode == InterfaceMode::Swappable;
        carrier.label.depth = self.label_interface.depth;
        carrier.label.supports = self.label_interface.supports;
        carrier.validate()?;
        Ok(carrier)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct EffectiveLabelInterface {
    pub size_x: u32,
    pub depth_micrometers: u64,
    pub boundaries_ppb: Vec<u32>,
    pub canonical_columns: Vec<String>,
}

#[derive(Debug)]
pub struct LoadedBin {
    pub path: PathBuf,
    pub source_version: u32,
    pub config: BinConfig,
}

#[derive(Debug, Clone)]
enum Track {
    Auto,
    Fraction(f64),
    Fixed(f64),
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct Face {
    side: Side,
    columns: [usize; 2],
    rows: [usize; 2],
}

#[derive(Debug)]
struct ResolvedEasyGrab {
    face: Face,
    radius_mm: f64,
}

impl LoadedBin {
    pub fn load(path: &Path) -> Result<Self> {
        let path = path
            .canonicalize()
            .with_context(|| format!("failed to resolve bin {}", path.display()))?;
        let source = fs::read_to_string(&path)
            .with_context(|| format!("failed to read bin {}", path.display()))?;
        let value: toml::Value = toml::from_str(&source)
            .with_context(|| format!("failed to parse bin {}", path.display()))?;
        let version = value
            .get("version")
            .and_then(toml::Value::as_integer)
            .with_context(|| format!("bin {} needs an integer version", path.display()))?;
        let config = match version {
            value if value == i64::from(BIN_CONFIG_VERSION) => {
                let config: BinConfig = toml::from_str(&source)
                    .with_context(|| format!("failed to parse bin {}", path.display()))?;
                config
            }
            value if value == i64::from(BIN_BODY_CONFIG_VERSION) => {
                let config: BinBodyFileConfig = toml::from_str(&source).with_context(|| {
                    format!("failed to parse constituent bin {}", path.display())
                })?;
                config.into_carrier()?
            }
            version => bail!(
                "unsupported bin TOML version {version}; expected {BIN_CONFIG_VERSION} or {BIN_BODY_CONFIG_VERSION}"
            ),
        };
        config
            .validate()
            .with_context(|| format!("invalid bin {}", path.display()))?;
        Ok(Self {
            path,
            source_version: version as u32,
            config,
        })
    }
}

impl BinConfig {
    pub fn validate(&self) -> Result<()> {
        if self.kind != ConfigKind::Bin {
            bail!("bin TOML kind must be \"bin\"");
        }
        if self.version != BIN_CONFIG_VERSION {
            bail!(
                "unsupported bin TOML version {}; expected {BIN_CONFIG_VERSION}",
                self.version
            );
        }
        for (axis, value) in ["X", "Y", "Z"].into_iter().zip(self.size) {
            if value == 0 {
                bail!("bin size {axis} must be at least one Gridfinity unit");
            }
        }
        if !self.bin.enabled {
            bail!(
                "bin TOML currently requires bin.enabled = true; the pinned model emits an unnamed helper body for base-only exports"
            );
        }
        let _ = length(
            &self.bin.additional_rim_expansion,
            "additional rim expansion",
        )?;
        let _ = length(&self.label.depth, "label depth")?;
        let _ = length(&self.label.embossing_clearance, "label embossing clearance")?;
        let _ = length(&self.label.embossing_inset, "label embossing inset")?;
        let default_radius = length(&self.easy_grab.radius, "easy-grab radius")?;
        if self.easy_grab.mode != EasyGrabMode::None && default_radius <= 0.0 {
            bail!("easy-grab radius must be greater than zero");
        }
        if !self.label.width_units.is_finite()
            || self.label.width_units <= 0.0
            || self.label.width_units > f64::from(self.size[0])
        {
            bail!(
                "label width-units must be greater than zero and no larger than bin width {}",
                self.size[0]
            );
        }
        if !self.print.max_overhang.is_finite()
            || self.print.max_overhang < 0.0
            || self.print.max_overhang > 90.0
        {
            bail!("print max-overhang must be between 0 and 90 degrees");
        }

        let columns = parse_tracks(&self.divider.columns, "column")?;
        let rows = parse_tracks(&self.divider.rows, "row")?;
        resolve_track_sizes(&columns, f64::from(self.size[0]) * GRIDFINITY_MM, "column")?;
        resolve_track_sizes(&rows, f64::from(self.size[1]) * GRIDFINITY_MM, "row")?;
        validate_merges(&self.divider, columns.len(), rows.len())?;
        let _ = self.resolved_easy_grabs()?;
        Ok(())
    }

    pub fn canonical_json(&self, _component: BinComponent) -> Result<String> {
        self.validate()?;
        serde_json::to_string(&self.canonical_value()?)
            .context("failed to serialize Gridfinity Ultimate configuration")
    }

    pub fn expected_parts(&self, component: BinComponent) -> Result<Vec<String>> {
        let mut parts = Vec::new();
        if self.bin.enabled {
            parts.push("Bin".to_owned());
            if self.bin.nesting && self.bin.swappable_rim {
                parts.push("SwappableRim".to_owned());
            }
            if self.bin.tub && self.label.enabled && self.label.swappable {
                parts.push("SwappableLabel".to_owned());
            }
        }
        if self.base.enabled {
            parts.push("Base".to_owned());
            if self.base.magnets && self.base.connector_pin {
                parts.push("ConnectorPin".to_owned());
            }
        }
        let Some(part_name) = component.part_name() else {
            return Ok(parts);
        };
        if !parts.iter().any(|part| part == part_name) {
            bail!(
                "configured component {part_name} does not exist; available parts: {}",
                if parts.is_empty() {
                    "(none)".to_owned()
                } else {
                    parts.join(", ")
                }
            );
        }
        Ok(vec![part_name.to_owned()])
    }

    pub fn supports_enabled(&self) -> Result<bool> {
        match self.label.supports {
            SupportsMode::Always => Ok(true),
            SupportsMode::Off => Ok(false),
            SupportsMode::Auto => {
                if self.divider.columns.len() < 3 {
                    return Ok(true);
                }
                let columns = parse_tracks(&self.divider.columns, "column")?;
                let sizes = resolve_track_sizes(
                    &columns,
                    f64::from(self.size[0]) * GRIDFINITY_MM,
                    "column",
                )?;
                Ok(sizes
                    .first()
                    .is_some_and(|size| *size > 0.75 * GRIDFINITY_MM)
                    || sizes
                        .last()
                        .is_some_and(|size| *size > 0.75 * GRIDFINITY_MM))
            }
        }
    }

    pub fn easy_grab_count(&self) -> Result<usize> {
        Ok(self.resolved_easy_grabs()?.len())
    }

    pub fn effective_label_interface(&self) -> Result<EffectiveLabelInterface> {
        if !self.bin.tub || !self.label.enabled || !self.label.swappable {
            bail!("bin does not define a swappable label interface");
        }
        let tracks = parse_tracks(&self.divider.columns, "column")?;
        let widths =
            resolve_track_sizes(&tracks, f64::from(self.size[0]) * GRIDFINITY_MM, "column")?;
        let rows = self.divider.rows.len();
        let components = Components::new(widths.len(), rows, &self.divider.merges);
        let total = widths.iter().sum::<f64>();
        let mut boundaries_ppb = Vec::new();
        let mut accumulated = 0.0;
        for (index, width) in widths.iter().enumerate() {
            accumulated += width;
            let has_wall = index + 1 < widths.len() && !components.same(index, 0, index + 1, 0);
            if has_wall {
                let boundary = (accumulated / total * 1_000_000_000.0).round();
                if !(1.0..1_000_000_000.0).contains(&boundary) {
                    bail!("resolved label divider boundary is outside the bin width");
                }
                let boundary = boundary as u32;
                if boundaries_ppb
                    .last()
                    .is_some_and(|previous| *previous >= boundary)
                {
                    bail!("resolved label divider boundaries are too close to normalize safely");
                }
                boundaries_ppb.push(boundary);
            }
        }
        let mut previous = 0u32;
        let mut segment_ticks = Vec::with_capacity(boundaries_ppb.len() + 1);
        for boundary in boundaries_ppb
            .iter()
            .copied()
            .chain(std::iter::once(1_000_000_000))
        {
            segment_ticks.push(boundary - previous);
            previous = boundary;
        }
        let divisor = segment_ticks.iter().copied().reduce(gcd).unwrap_or(1);
        let canonical_columns = segment_ticks
            .into_iter()
            .map(|width| format!("{}fr", width / divisor))
            .collect();
        let depth_mm = length(&self.label.depth, "label depth")?;
        Ok(EffectiveLabelInterface {
            size_x: self.size[0],
            depth_micrometers: (depth_mm * 1000.0).round() as u64,
            boundaries_ppb,
            canonical_columns,
        })
    }

    fn canonical_value(&self) -> Result<Value> {
        let additional_expansion = length(
            &self.bin.additional_rim_expansion,
            "additional rim expansion",
        )?;
        let label_depth = length(&self.label.depth, "label depth")?;
        let embossing_clearance =
            length(&self.label.embossing_clearance, "label embossing clearance")?;
        let embossing_inset = length(&self.label.embossing_inset, "label embossing inset")?;
        let columns = parse_tracks(&self.divider.columns, "column")?;
        let rows = parse_tracks(&self.divider.rows, "row")?;

        let mut divider = Map::new();
        divider.insert(
            "columns".to_owned(),
            Value::Array(columns.iter().map(track_value).collect::<Result<_>>()?),
        );
        divider.insert(
            "rows".to_owned(),
            Value::Array(rows.iter().map(track_value).collect::<Result<_>>()?),
        );
        divider.insert(
            "merges".to_owned(),
            Value::Array(
                self.divider
                    .merges
                    .iter()
                    .map(|merge| {
                        json!({
                            "cols": merge.columns,
                            "rows": merge.rows,
                        })
                    })
                    .collect(),
            ),
        );
        let easy_grabs = self.resolved_easy_grabs()?;
        if !easy_grabs.is_empty() {
            divider.insert(
                "easygrab".to_owned(),
                Value::Array(
                    easy_grabs
                        .iter()
                        .map(|entry| {
                            json!({
                                "side": entry.face.side.as_str(),
                                "cols": entry.face.columns,
                                "rows": entry.face.rows,
                                "radius": format_mm(entry.radius_mm),
                            })
                        })
                        .collect(),
                ),
            );
        }

        let mut value = Map::new();
        value.insert("base_enable".to_owned(), json!(self.base.enabled));
        value.insert(
            "base_magnets_connector_cutouts_enable".to_owned(),
            json!(self.base.connector_cutouts),
        );
        value.insert(
            "base_magnets_connector_pin_enable".to_owned(),
            json!(self.base.connector_pin),
        );
        value.insert("base_magnets_enable".to_owned(), json!(self.base.magnets));
        value.insert(
            "base_rounded_corners_enable".to_owned(),
            json!(self.base.rounded_corners),
        );
        value.insert("bin_enable".to_owned(), json!(self.bin.enabled));
        value.insert("bin_nesting_enable".to_owned(), json!(self.bin.nesting));
        value.insert(
            "bin_nesting_swappable_rim_enable".to_owned(),
            json!(self.bin.swappable_rim),
        );
        value.insert(
            "bin_nesting_swappable_rim_spring_compensation_enable".to_owned(),
            json!(self.bin.spring_compensation),
        );
        value.insert(
            "bin_tub_easygrab_enable".to_owned(),
            json!(self.easy_grab.mode != EasyGrabMode::None),
        );
        value.insert("bin_tub_enable".to_owned(), json!(self.bin.tub));
        value.insert(
            "bin_tub_label_depth".to_owned(),
            json!(format_meter(label_depth)),
        );
        value.insert("bin_tub_label_enable".to_owned(), json!(self.label.enabled));
        value.insert(
            "bin_tub_label_is_fullwidth".to_owned(),
            json!(self.label.full_width),
        );
        value.insert(
            "bin_tub_label_is_swappable".to_owned(),
            json!(self.label.swappable),
        );
        value.insert(
            "bin_tub_label_width_units".to_owned(),
            number_value(self.label.width_units),
        );
        value.insert(
            "max_print_overhang".to_owned(),
            json!(format!("{} deg", compact(self.print.max_overhang))),
        );
        value.insert(
            "bin_nesting_swappable_rim_spring_compensation_additional_rim_expansion".to_owned(),
            json!(format_meter(additional_expansion)),
        );
        value.insert("size_x_units".to_owned(), json!(self.size[0]));
        value.insert("size_y_units".to_owned(), json!(self.size[1]));
        value.insert("size_z_units".to_owned(), json!(self.size[2]));
        value.insert("bin_tub_divider_config".to_owned(), Value::Object(divider));
        value.insert(
            "bin_tub_label_swappable_supports_enable".to_owned(),
            json!(self.supports_enabled()?),
        );
        value.insert(
            "bin_tub_label_swappable_embossing_clearance".to_owned(),
            json!(format_meter(embossing_clearance)),
        );
        value.insert(
            "bin_tub_label_swappable_embossing_inset_height".to_owned(),
            json!(format_meter(embossing_inset)),
        );
        Ok(Value::Object(value))
    }

    fn resolved_easy_grabs(&self) -> Result<Vec<ResolvedEasyGrab>> {
        let valid_faces = all_faces(&self.divider)?;
        let default_radius = length(&self.easy_grab.radius, "easy-grab radius")?;
        match self.easy_grab.mode {
            EasyGrabMode::None => Ok(Vec::new()),
            EasyGrabMode::All => Ok(valid_faces
                .into_iter()
                .filter(|face| face.side == self.easy_grab.side)
                .map(|face| ResolvedEasyGrab {
                    face,
                    radius_mm: default_radius,
                })
                .collect()),
            EasyGrabMode::Custom => {
                let valid = valid_faces.into_iter().collect::<BTreeSet<_>>();
                self.easy_grab
                    .faces
                    .iter()
                    .enumerate()
                    .map(|(index, entry)| {
                        let face = Face {
                            side: entry.side,
                            columns: entry.columns,
                            rows: entry.rows,
                        };
                        if !valid.contains(&face) {
                            bail!(
                                "easy-grab face {} is not a complete, capped divider wall face",
                                index + 1
                            );
                        }
                        let radius_mm = match &entry.radius {
                            Some(radius) => {
                                length(radius, &format!("easy-grab face {} radius", index + 1))?
                            }
                            None => default_radius,
                        };
                        if radius_mm <= 0.0 {
                            bail!(
                                "easy-grab face {} radius must be greater than zero",
                                index + 1
                            );
                        }
                        Ok(ResolvedEasyGrab { face, radius_mm })
                    })
                    .collect()
            }
        }
    }
}

fn length(value: &str, label: &str) -> Result<f64> {
    parse_length_mm(value).with_context(|| format!("invalid {label} {value:?}"))
}

fn parse_tracks(values: &[String], axis: &str) -> Result<Vec<Track>> {
    if values.is_empty() {
        bail!("divider {axis}s must contain at least one track");
    }
    values
        .iter()
        .enumerate()
        .map(|(index, value)| parse_track(value, &format!("{axis} {}", index + 1)))
        .collect()
}

fn parse_track(value: &str, label: &str) -> Result<Track> {
    let value = value.trim();
    if value.eq_ignore_ascii_case("auto") {
        return Ok(Track::Auto);
    }
    if let Some(weight) = value.to_ascii_lowercase().strip_suffix("fr") {
        return fraction(weight.trim(), label);
    }
    if value.bytes().any(|byte| byte.is_ascii_alphabetic()) {
        return Ok(Track::Fixed(length(value, label)?));
    }
    fraction(value, label)
}

fn fraction(value: &str, label: &str) -> Result<Track> {
    let value: f64 = value
        .parse()
        .with_context(|| format!("invalid {label} fraction {value:?}"))?;
    if !value.is_finite() || value <= 0.0 {
        bail!("{label} fraction must be finite and greater than zero");
    }
    Ok(Track::Fraction(value))
}

fn resolve_track_sizes(tracks: &[Track], total: f64, axis: &str) -> Result<Vec<f64>> {
    let fixed_total = tracks
        .iter()
        .filter_map(|track| match track {
            Track::Fixed(value) => Some(*value),
            _ => None,
        })
        .sum::<f64>();
    let flex_total = tracks
        .iter()
        .map(|track| match track {
            Track::Auto => 1.0,
            Track::Fraction(value) => *value,
            Track::Fixed(_) => 0.0,
        })
        .sum::<f64>();
    let remaining = total - fixed_total;
    if remaining < -1e-9 {
        bail!("fixed divider {axis} sizes exceed {} mm", compact(total));
    }
    if flex_total == 0.0 && remaining > 1e-9 {
        bail!(
            "fixed divider {axis} sizes do not fill the bin and there are no auto/fractional tracks"
        );
    }
    Ok(tracks
        .iter()
        .map(|track| match track {
            Track::Fixed(value) => *value,
            Track::Auto => remaining / flex_total,
            Track::Fraction(value) => remaining * value / flex_total,
        })
        .collect())
}

fn validate_merges(divider: &DividerConfig, columns: usize, rows: usize) -> Result<()> {
    for (index, merge) in divider.merges.iter().enumerate() {
        if merge.columns[0] > merge.columns[1] || merge.columns[1] >= columns {
            bail!("divider merge {} columns are out of range", index + 1);
        }
        if merge.rows[0] > merge.rows[1] || merge.rows[1] >= rows {
            bail!("divider merge {} rows are out of range", index + 1);
        }
    }
    Ok(())
}

fn track_value(track: &Track) -> Result<Value> {
    match track {
        Track::Auto => Ok(json!("auto")),
        Track::Fraction(value) => Ok(number_value(round(*value, 9))),
        Track::Fixed(value) => Ok(json!(format!("{} mm", compact(round(*value, 6))))),
    }
}

fn all_faces(divider: &DividerConfig) -> Result<Vec<Face>> {
    let columns = divider.columns.len();
    let rows = divider.rows.len();
    validate_merges(divider, columns, rows)?;
    let components = Components::new(columns, rows, &divider.merges);
    let same = |column: usize, row: usize, other_column: isize, other_row: isize| {
        if other_column < 0
            || other_row < 0
            || other_column >= columns as isize
            || other_row >= rows as isize
        {
            false
        } else {
            components.same(column, row, other_column as usize, other_row as usize)
        }
    };
    let mut faces = Vec::new();

    for row in 0..rows {
        for (side, offset) in [(Side::South, 1isize), (Side::North, -1isize)] {
            let mut column = 0;
            while column < columns {
                if same(column, row, column as isize, row as isize + offset) {
                    column += 1;
                    continue;
                }
                let start = column;
                column += 1;
                while column < columns
                    && !same(column, row, column as isize, row as isize + offset)
                    && same(column - 1, row, column as isize, row as isize)
                {
                    column += 1;
                }
                let end = column - 1;
                let capped = (start == 0 || !same(start, row, start as isize - 1, row as isize))
                    && (end + 1 == columns || !same(end, row, end as isize + 1, row as isize));
                if capped {
                    faces.push(Face {
                        side,
                        columns: [start, end],
                        rows: [row, row],
                    });
                }
            }
        }
    }

    for column in 0..columns {
        for (side, offset) in [(Side::East, 1isize), (Side::West, -1isize)] {
            let mut row = 0;
            while row < rows {
                if same(column, row, column as isize + offset, row as isize) {
                    row += 1;
                    continue;
                }
                let start = row;
                row += 1;
                while row < rows
                    && !same(column, row, column as isize + offset, row as isize)
                    && same(column, row - 1, column as isize, row as isize)
                {
                    row += 1;
                }
                let end = row - 1;
                let capped = (start == 0
                    || !same(column, start, column as isize, start as isize - 1))
                    && (end + 1 == rows || !same(column, end, column as isize, end as isize + 1));
                if capped {
                    faces.push(Face {
                        side,
                        columns: [column, column],
                        rows: [start, end],
                    });
                }
            }
        }
    }
    Ok(faces)
}

struct Components {
    columns: usize,
    parent: Vec<usize>,
}

impl Components {
    fn new(columns: usize, rows: usize, merges: &[DividerMerge]) -> Self {
        let mut result = Self {
            columns,
            parent: (0..columns * rows).collect(),
        };
        for merge in merges {
            let base = result.index(merge.columns[0], merge.rows[0]);
            for row in merge.rows[0]..=merge.rows[1] {
                for column in merge.columns[0]..=merge.columns[1] {
                    let cell = result.index(column, row);
                    result.union(cell, base);
                }
            }
        }
        result
    }

    fn index(&self, column: usize, row: usize) -> usize {
        row * self.columns + column
    }

    fn root(&self, mut index: usize) -> usize {
        while self.parent[index] != index {
            index = self.parent[index];
        }
        index
    }

    fn union(&mut self, left: usize, right: usize) {
        let left = self.root(left);
        let right = self.root(right);
        if left != right {
            self.parent[left] = right;
        }
    }

    fn same(&self, c1: usize, r1: usize, c2: usize, r2: usize) -> bool {
        self.root(self.index(c1, r1)) == self.root(self.index(c2, r2))
    }
}

fn number_value(value: f64) -> Value {
    if value.fract() == 0.0 && value >= 0.0 && value <= u64::MAX as f64 {
        json!(value as u64)
    } else {
        json!(value)
    }
}

fn format_meter(mm: f64) -> String {
    format!("{} meter", compact(round(mm / 1000.0, 6)))
}

fn format_mm(mm: f64) -> String {
    format!("{} mm", compact(round(mm, 4)))
}

fn gcd(mut left: u32, mut right: u32) -> u32 {
    while right != 0 {
        let remainder = left % right;
        left = right;
        right = remainder;
    }
    left.max(1)
}

fn round(value: f64, digits: i32) -> f64 {
    let factor = 10f64.powi(digits);
    (value * factor).round() / factor
}

fn compact(value: f64) -> String {
    let value = format!("{value:.9}");
    value.trim_end_matches('0').trim_end_matches('.').to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_bin() -> BinConfig {
        toml::from_str(include_str!("../tests/fixtures/bin/default.toml")).unwrap()
    }

    #[test]
    fn default_configuration_matches_designer_semantics() {
        let config = default_bin();
        config.validate().unwrap();
        let value: Value =
            serde_json::from_str(&config.canonical_json(BinComponent::All).unwrap()).unwrap();
        let expected: Value =
            serde_json::from_str(include_str!("../tests/fixtures/bin/default.json")).unwrap();
        assert_eq!(value, expected);
        assert_eq!(value["size_x_units"], 2);
        assert_eq!(value["size_y_units"], 2);
        assert_eq!(value["bin_tub_label_depth"], "0.01 meter");
        assert_eq!(
            value["bin_tub_label_swappable_embossing_clearance"],
            "0.0004 meter"
        );
        assert_eq!(value["bin_tub_label_swappable_supports_enable"], false);
        assert_eq!(
            value["bin_tub_divider_config"]["easygrab"]
                .as_array()
                .unwrap()
                .len(),
            6
        );
        assert_eq!(
            config.expected_parts(BinComponent::All).unwrap(),
            [
                "Bin",
                "SwappableRim",
                "SwappableLabel",
                "Base",
                "ConnectorPin"
            ]
        );
    }

    #[test]
    fn one_by_one_enables_automatic_label_supports() {
        let config: BinConfig = toml::from_str(
            r#"
kind = "bin"
version = 1
size = [1, 1, 6]

[divider]
columns = ["auto"]
rows = ["auto"]
"#,
        )
        .unwrap();
        assert!(config.supports_enabled().unwrap());
        assert_eq!(config.easy_grab_count().unwrap(), 1);
    }

    #[test]
    fn validates_tracks_merges_and_custom_easy_grabs() {
        let config: BinConfig = toml::from_str(
            r#"
kind = "bin"
version = 1
size = [2, 1, 4]

[divider]
columns = ["21mm", "1fr", "2fr"]
rows = ["auto"]

[[divider.merges]]
columns = [1, 2]
rows = [0, 0]

[easy-grab]
mode = "custom"
radius = "18mm"

[[easy-grab.faces]]
side = "south"
columns = [1, 2]
rows = [0, 0]
radius = "12mm"
"#,
        )
        .unwrap();
        config.validate().unwrap();
        assert_eq!(config.easy_grab_count().unwrap(), 1);
    }

    #[test]
    fn rejects_invalid_custom_face() {
        let mut config = default_bin();
        config.easy_grab.mode = EasyGrabMode::Custom;
        config.easy_grab.faces.push(EasyGrabFace {
            side: Side::South,
            columns: [0, 1],
            rows: [0, 0],
            radius: None,
        });
        let error = config.validate().unwrap_err().to_string();
        assert!(error.contains("complete, capped"));
    }

    #[test]
    fn rejects_base_only_bin_files_before_export() {
        let mut config = default_bin();
        config.bin.enabled = false;
        let error = config.validate().unwrap_err().to_string();
        assert!(error.contains("base-only"));
        assert!(error.contains("helper body"));
    }

    #[test]
    fn component_selection_uses_exact_named_manifests_without_mutating_geometry() {
        let config = default_bin();
        let bin: Value =
            serde_json::from_str(&config.canonical_json(BinComponent::Bin).unwrap()).unwrap();
        assert_eq!(bin["base_enable"], true);
        assert_eq!(config.expected_parts(BinComponent::Bin).unwrap(), ["Bin"]);
        assert_eq!(
            config.expected_parts(BinComponent::SwappableRim).unwrap(),
            ["SwappableRim"]
        );
    }

    #[test]
    fn normalizes_equivalent_first_row_partitions() {
        let mut merged = default_bin();
        merged.divider.columns = vec!["auto".to_owned(); 4];
        merged.divider.merges = vec![
            DividerMerge {
                columns: [0, 1],
                rows: [0, 0],
            },
            DividerMerge {
                columns: [2, 3],
                rows: [0, 0],
            },
        ];
        let mut two_columns = default_bin();
        two_columns.divider.columns = vec!["1fr".to_owned(), "1fr".to_owned()];

        let merged = merged.effective_label_interface().unwrap();
        let two_columns = two_columns.effective_label_interface().unwrap();
        assert_eq!(merged.boundaries_ppb, vec![500_000_000]);
        assert_eq!(merged, two_columns);
        assert_eq!(merged.canonical_columns, ["1fr", "1fr"]);
    }

    #[test]
    fn loads_constituent_bin_version_two() {
        let config: BinBodyFileConfig = toml::from_str(
            r#"
kind = "bin"
version = 2
size = [2, 1, 6]

[rim-interface]
mode = "swappable"

[label-interface]
mode = "swappable"
depth = "12mm"
supports = "off"

[divider]
columns = ["1fr", "1fr"]
rows = ["auto"]
"#,
        )
        .unwrap();
        let carrier = config.into_carrier().unwrap();
        assert!(!carrier.base.enabled);
        assert!(carrier.bin.swappable_rim);
        assert!(carrier.label.swappable);
        assert_eq!(carrier.label.depth, "12mm");
        assert_eq!(
            carrier.expected_parts(BinComponent::All).unwrap(),
            ["Bin", "SwappableRim", "SwappableLabel"]
        );
    }
}
