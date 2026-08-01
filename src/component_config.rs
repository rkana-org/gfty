use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};

use crate::{
    bin_config::{
        BaseConfig as ModelBaseConfig, BinBodyConfig, BinLabelConfig, DividerConfig,
        EasyGrabConfig, EasyGrabMode, EffectiveLabelInterface, GridfinityConfig, LoadedBin,
        PrintConfig,
    },
    config::{ConfigKind, parse_length_mm},
};

pub const COMPONENT_CONFIG_VERSION: u32 = 1;

#[derive(Debug, Clone)]
pub struct ResolvedGridfinityExport {
    pub source_path: PathBuf,
    pub carrier: GridfinityConfig,
    pub part_names: Vec<String>,
    pub request_key: String,
    pub description: String,
}

impl ResolvedGridfinityExport {
    pub fn gridfinity_json(&self) -> Result<String> {
        self.carrier.canonical_json()
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct BaseFileConfig {
    pub kind: ConfigKind,
    pub version: u32,
    pub size: [u32; 2],
    #[serde(default)]
    pub rounded_corners: bool,
    #[serde(default)]
    pub magnets: BaseMagnetsConfig,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields, rename_all = "kebab-case")]
pub struct BaseMagnetsConfig {
    pub enabled: bool,
    pub connector_cutouts: bool,
}

impl Default for BaseMagnetsConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            connector_cutouts: true,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct RimFileConfig {
    pub kind: ConfigKind,
    pub version: u32,
    pub size: [u32; 2],
    #[serde(default = "default_true")]
    pub spring_compensation: bool,
    #[serde(default = "default_expansion")]
    pub additional_expansion: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct SwappableLabelFileConfig {
    pub kind: ConfigKind,
    pub version: u32,
    pub bin: String,
    #[serde(default)]
    pub embossing: EmbossingConfig,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields, rename_all = "kebab-case")]
pub struct EmbossingConfig {
    pub clearance: String,
    pub inset: String,
}

impl Default for EmbossingConfig {
    fn default() -> Self {
        Self {
            clearance: "0.4mm".to_owned(),
            inset: "0mm".to_owned(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct BinSetFileConfig {
    pub kind: ConfigKind,
    pub version: u32,
    pub bin: String,
    #[serde(default)]
    pub base: Option<String>,
    #[serde(default)]
    pub rim: Option<String>,
    #[serde(default)]
    pub swappable_label: Option<String>,
    #[serde(default)]
    pub connector_pin: bool,
}

fn default_true() -> bool {
    true
}

fn default_expansion() -> String {
    "0mm".to_owned()
}

pub fn load_bin(path: &Path) -> Result<ResolvedGridfinityExport> {
    let bin = LoadedBin::load(path)?;
    let part_names = vec!["Bin".to_owned()];
    let gridfinity = serde_json::from_str::<serde_json::Value>(&bin.config.canonical_json()?)?;
    resolved(
        bin.path,
        bin.config,
        part_names.clone(),
        "bin",
        json!({
            "contract": "gfty-bin-export/v1",
            "gridfinity": gridfinity,
            "parts": part_names,
        }),
    )
}

pub fn load_base(path: &Path) -> Result<ResolvedGridfinityExport> {
    let (path, config): (_, BaseFileConfig) = load_toml(path, "base")?;
    validate_header(config.kind, ConfigKind::Base, config.version, "base")?;
    validate_xy(config.size, "base")?;
    let mut carrier = carrier([config.size[0], config.size[1], 1]);
    apply_base(&mut carrier, &config, false);
    carrier.validate()?;
    resolved(
        path,
        carrier,
        vec!["Base".to_owned()],
        "base",
        json!({
            "contract": "gfty-base/v1",
            "size": config.size,
            "rounded-corners": config.rounded_corners,
            "magnets": config.magnets,
        }),
    )
}

pub fn load_rim(path: &Path) -> Result<ResolvedGridfinityExport> {
    let (path, config): (_, RimFileConfig) = load_toml(path, "rim")?;
    validate_header(config.kind, ConfigKind::Rim, config.version, "rim")?;
    validate_xy(config.size, "rim")?;
    let _ = parse_length_mm(&config.additional_expansion)
        .context("invalid rim additional-expansion")?;
    let mut carrier = carrier([config.size[0], config.size[1], 1]);
    carrier.bin.nesting = true;
    carrier.bin.swappable_rim = true;
    carrier.bin.spring_compensation = config.spring_compensation;
    carrier.bin.additional_rim_expansion = config.additional_expansion.clone();
    carrier.validate()?;
    resolved(
        path,
        carrier,
        vec!["SwappableRim".to_owned()],
        "swappable rim",
        json!({
            "contract": "gfty-rim/v1",
            "size": config.size,
            "spring-compensation": config.spring_compensation,
            "additional-expansion-micrometers": micrometers(&config.additional_expansion)?,
        }),
    )
}

pub fn load_swappable_label(path: &Path) -> Result<ResolvedGridfinityExport> {
    let (path, config): (_, SwappableLabelFileConfig) = load_toml(path, "swappable label")?;
    validate_header(
        config.kind,
        ConfigKind::SwappableLabel,
        config.version,
        "swappable label",
    )?;
    let bin_path = resolve_reference(&path, &config.bin);
    let bin = LoadedBin::load(&bin_path).with_context(|| {
        format!(
            "failed to load bin referenced by swappable label {}",
            path.display()
        )
    })?;
    let interface = bin.config.effective_label_interface()?;
    let carrier = label_carrier(&interface, &config.embossing)?;
    resolved(
        path,
        carrier,
        vec!["SwappableLabel".to_owned()],
        "swappable label",
        label_key_value(&interface, &config.embossing)?,
    )
}

pub fn load_bin_set(path: &Path) -> Result<ResolvedGridfinityExport> {
    let (path, config): (_, BinSetFileConfig) = load_toml(path, "bin set")?;
    validate_header(config.kind, ConfigKind::BinSet, config.version, "bin set")?;
    let bin_path = resolve_reference(&path, &config.bin);
    let bin = LoadedBin::load(&bin_path)
        .with_context(|| format!("failed to load bin referenced by set {}", path.display()))?;
    let mut carrier = bin.config.clone();
    carrier.base.enabled = false;
    carrier.base.connector_pin = false;
    let mut part_names = vec!["Bin".to_owned()];
    let bin_xy = [carrier.size[0], carrier.size[1]];

    if let Some(base_reference) = &config.base {
        let base_path = resolve_reference(&path, base_reference);
        let (_, base): (_, BaseFileConfig) = load_toml(&base_path, "base")?;
        validate_header(base.kind, ConfigKind::Base, base.version, "base")?;
        if base.size != bin_xy {
            bail!(
                "base {} size {:?} is incompatible with bin size {:?}",
                base_path.display(),
                base.size,
                bin_xy
            );
        }
        apply_base(&mut carrier, &base, config.connector_pin);
        part_names.push("Base".to_owned());
        if config.connector_pin {
            if !base.magnets.enabled || !base.magnets.connector_cutouts {
                bail!("connector-pin requires a magnetic base with connector cutouts");
            }
            part_names.push("ConnectorPin".to_owned());
        }
    } else if config.connector_pin {
        bail!("bin set connector-pin requires a base reference");
    }

    if carrier.bin.nesting && carrier.bin.swappable_rim {
        let rim_reference = config
            .rim
            .as_deref()
            .context("bin has a swappable rim interface, so the bin set needs a rim reference")?;
        let rim_path = resolve_reference(&path, rim_reference);
        let (_, rim): (_, RimFileConfig) = load_toml(&rim_path, "rim")?;
        validate_header(rim.kind, ConfigKind::Rim, rim.version, "rim")?;
        if rim.size != bin_xy {
            bail!(
                "rim {} size {:?} is incompatible with bin size {:?}",
                rim_path.display(),
                rim.size,
                bin_xy
            );
        }
        carrier.bin.spring_compensation = rim.spring_compensation;
        carrier.bin.additional_rim_expansion = rim.additional_expansion;
        part_names.push("SwappableRim".to_owned());
    } else if config.rim.is_some() {
        bail!("bin set supplies a rim but the bin has no swappable rim interface");
    }

    if carrier.bin.tub && carrier.label.enabled && carrier.label.swappable {
        let label_reference = config.swappable_label.as_deref().context(
            "bin has a swappable label interface, so the bin set needs a swappable-label reference",
        )?;
        let label_path = resolve_reference(&path, label_reference);
        let (_, label): (_, SwappableLabelFileConfig) = load_toml(&label_path, "swappable label")?;
        validate_header(
            label.kind,
            ConfigKind::SwappableLabel,
            label.version,
            "swappable label",
        )?;
        let prototype_path = resolve_reference(&label_path, &label.bin);
        let prototype = LoadedBin::load(&prototype_path).with_context(|| {
            format!(
                "failed to load prototype bin referenced by {}",
                label_path.display()
            )
        })?;
        let expected = carrier.effective_label_interface()?;
        let actual = prototype.config.effective_label_interface()?;
        if actual != expected {
            bail!(
                "swappable label {} is incompatible with bin {}; expected interface {}, received {}",
                label_path.display(),
                bin_path.display(),
                interface_summary(&expected),
                interface_summary(&actual)
            );
        }
        carrier.label.embossing_clearance = label.embossing.clearance;
        carrier.label.embossing_inset = label.embossing.inset;
        part_names.push("SwappableLabel".to_owned());
    } else if config.swappable_label.is_some() {
        bail!("bin set supplies a swappable label but the bin has no swappable label interface");
    }

    carrier.validate()?;
    let key_value = json!({
        "contract": "gfty-bin-set/v1",
        "gridfinity": serde_json::from_str::<serde_json::Value>(&carrier.canonical_json()?)?,
        "parts": part_names,
    });
    resolved(path, carrier, part_names, "bin set", key_value)
}

pub fn connector_pin() -> Result<ResolvedGridfinityExport> {
    let mut carrier = carrier([1, 1, 1]);
    carrier.base.enabled = true;
    carrier.base.magnets = true;
    carrier.base.connector_cutouts = true;
    carrier.base.connector_pin = true;
    carrier.validate()?;
    resolved(
        PathBuf::from("<connector-pin>"),
        carrier,
        vec!["ConnectorPin".to_owned()],
        "connector pin",
        json!({ "contract": "gfty-connector-pin/v1" }),
    )
}

fn carrier(size: [u32; 3]) -> GridfinityConfig {
    let base = ModelBaseConfig {
        enabled: false,
        connector_pin: false,
        ..ModelBaseConfig::default()
    };
    let label = BinLabelConfig {
        enabled: false,
        ..BinLabelConfig::default()
    };
    let easy_grab = EasyGrabConfig {
        mode: EasyGrabMode::None,
        ..EasyGrabConfig::default()
    };
    GridfinityConfig {
        size,
        base,
        bin: BinBodyConfig::default(),
        label,
        divider: DividerConfig {
            columns: vec!["auto".to_owned()],
            rows: vec!["auto".to_owned()],
            merges: Vec::new(),
        },
        easy_grab,
        print: PrintConfig::default(),
    }
}

fn apply_base(carrier: &mut GridfinityConfig, base: &BaseFileConfig, connector_pin: bool) {
    carrier.base.enabled = true;
    carrier.base.rounded_corners = base.rounded_corners;
    carrier.base.magnets = base.magnets.enabled;
    carrier.base.connector_cutouts = base.magnets.connector_cutouts;
    carrier.base.connector_pin = connector_pin;
}

fn label_carrier(
    interface: &EffectiveLabelInterface,
    embossing: &EmbossingConfig,
) -> Result<GridfinityConfig> {
    let mut carrier = carrier([interface.size_x, 1, 6]);
    carrier.bin.nesting = false;
    carrier.bin.tub = true;
    carrier.label.enabled = true;
    carrier.label.swappable = true;
    carrier.label.depth = format_micrometers(interface.depth_micrometers);
    carrier.label.supports = crate::bin_config::SupportsMode::Off;
    carrier.label.embossing_clearance = embossing.clearance.clone();
    carrier.label.embossing_inset = embossing.inset.clone();
    carrier.divider.columns = interface.canonical_columns.clone();
    carrier.divider.rows = vec!["auto".to_owned()];
    carrier.divider.merges.clear();
    carrier.validate()?;
    Ok(carrier)
}

fn label_key_value(
    interface: &EffectiveLabelInterface,
    embossing: &EmbossingConfig,
) -> Result<serde_json::Value> {
    Ok(json!({
        "contract": "gfty-swappable-label/v1",
        "interface": interface,
        "embossing-clearance-micrometers": micrometers(&embossing.clearance)?,
        "embossing-inset-micrometers": micrometers(&embossing.inset)?,
    }))
}

fn resolved(
    source_path: PathBuf,
    carrier: GridfinityConfig,
    part_names: Vec<String>,
    description: &str,
    key_value: serde_json::Value,
) -> Result<ResolvedGridfinityExport> {
    let canonical =
        serde_json::to_vec(&key_value).context("failed to serialize request identity")?;
    let request_key = hex(&Sha256::digest(canonical));
    Ok(ResolvedGridfinityExport {
        source_path,
        carrier,
        part_names,
        request_key,
        description: description.to_owned(),
    })
}

fn load_toml<T: for<'de> Deserialize<'de>>(path: &Path, description: &str) -> Result<(PathBuf, T)> {
    let path = path
        .canonicalize()
        .with_context(|| format!("failed to resolve {description} {}", path.display()))?;
    let source = fs::read_to_string(&path)
        .with_context(|| format!("failed to read {description} {}", path.display()))?;
    let config = toml::from_str(&source)
        .with_context(|| format!("failed to parse {description} {}", path.display()))?;
    Ok((path, config))
}

fn validate_header(
    actual_kind: ConfigKind,
    expected_kind: ConfigKind,
    version: u32,
    description: &str,
) -> Result<()> {
    if actual_kind != expected_kind {
        bail!("{description} TOML has the wrong kind");
    }
    if version != COMPONENT_CONFIG_VERSION {
        bail!(
            "unsupported {description} TOML version {version}; expected {COMPONENT_CONFIG_VERSION}"
        );
    }
    Ok(())
}

fn validate_xy(size: [u32; 2], description: &str) -> Result<()> {
    if size[0] == 0 || size[1] == 0 {
        bail!("{description} X and Y sizes must be at least one Gridfinity unit");
    }
    Ok(())
}

fn resolve_reference(owner: &Path, reference: &str) -> PathBuf {
    let reference = Path::new(reference);
    if reference.is_absolute() {
        reference.to_owned()
    } else {
        owner
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join(reference)
    }
}

fn micrometers(value: &str) -> Result<u64> {
    let millimeters = parse_length_mm(value)?;
    Ok((millimeters * 1000.0).round() as u64)
}

fn format_micrometers(value: u64) -> String {
    if value.is_multiple_of(1000) {
        format!("{}mm", value / 1000)
    } else {
        format!("{}mm", value as f64 / 1000.0)
    }
}

fn interface_summary(interface: &EffectiveLabelInterface) -> String {
    format!(
        "x={}, depth={}um, boundaries={:?}",
        interface.size_x, interface.depth_micrometers, interface.boundaries_ppb
    )
}

fn hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut result = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        result.push(char::from(HEX[usize::from(byte >> 4)]));
        result.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn write(path: &Path, contents: &str) {
        fs::write(path, contents).unwrap();
    }

    #[test]
    fn labels_from_equivalent_bins_have_the_same_request_key() {
        let directory = tempdir().unwrap();
        let first = directory.path().join("first.toml");
        let second = directory.path().join("second.toml");
        write(
            &first,
            r#"
kind = "bin"
version = 2
size = [2, 2, 6]
[divider]
columns = ["auto", "auto", "auto", "auto"]
rows = ["auto", "auto"]
[[divider.merges]]
columns = [0, 1]
rows = [0, 0]
[[divider.merges]]
columns = [2, 3]
rows = [0, 0]
"#,
        );
        write(
            &second,
            r#"
kind = "bin"
version = 2
size = [2, 8, 3]
[divider]
columns = ["1fr", "1fr"]
rows = ["auto", "auto", "auto"]
"#,
        );
        let label_a = directory.path().join("a.toml");
        let label_b = directory.path().join("b.toml");
        write(
            &label_a,
            "kind = \"swappable-label\"\nversion = 1\nbin = \"first.toml\"\n",
        );
        write(
            &label_b,
            "kind = \"swappable-label\"\nversion = 1\nbin = \"second.toml\"\n",
        );
        let a = load_swappable_label(&label_a).unwrap();
        let b = load_swappable_label(&label_b).unwrap();
        assert_eq!(a.request_key, b.request_key);
        assert_eq!(a.gridfinity_json().unwrap(), b.gridfinity_json().unwrap());
    }

    #[test]
    fn set_accepts_a_label_derived_from_a_compatible_bin() {
        let directory = tempdir().unwrap();
        write(
            &directory.path().join("bin-a.toml"),
            r#"
kind = "bin"
version = 2
size = [2, 1, 6]
[divider]
columns = ["auto", "auto"]
rows = ["auto"]
"#,
        );
        write(
            &directory.path().join("bin-b.toml"),
            r#"
kind = "bin"
version = 2
size = [2, 5, 8]
[divider]
columns = ["1fr", "1fr"]
rows = ["auto", "auto"]
"#,
        );
        write(
            &directory.path().join("base.toml"),
            "kind = \"base\"\nversion = 1\nsize = [2, 5]\n",
        );
        write(
            &directory.path().join("rim.toml"),
            "kind = \"rim\"\nversion = 1\nsize = [2, 5]\n",
        );
        write(
            &directory.path().join("label.toml"),
            "kind = \"swappable-label\"\nversion = 1\nbin = \"bin-a.toml\"\n",
        );
        write(
            &directory.path().join("set.toml"),
            r#"
kind = "bin-set"
version = 1
bin = "bin-b.toml"
base = "base.toml"
rim = "rim.toml"
swappable-label = "label.toml"
connector-pin = true
"#,
        );
        let set = load_bin_set(&directory.path().join("set.toml")).unwrap();
        assert_eq!(
            set.part_names,
            [
                "Bin",
                "Base",
                "ConnectorPin",
                "SwappableRim",
                "SwappableLabel"
            ]
        );
    }
}
