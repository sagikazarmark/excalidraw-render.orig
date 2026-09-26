//! Recompute representative comparisons from retained RGBA/PNG observations.
//! Stored pass/fail booleans are never used as a substitute for pixel comparison.
use super::*;
use crate::cli::ControlsArgs;
use excalidraw_render_compare::{
    compare, CanonicalRgba8, ComparisonMode, Crop, RegionReport, SsimOutcome, VisualClass,
};

#[derive(Clone, Copy)]
pub(super) struct Comparison {
    pub id: &'static str,
    pub format: &'static str,
    pub profile: Option<&'static str>,
    left: &'static str,
    right: &'static str,
    mode: ComparisonMode,
}

pub(super) const COMPARISONS: [Comparison; 5] = [
    Comparison {
        id: "svg-chromium",
        format: "svg",
        profile: Some("canonical-svg-chromium-v1"),
        left: "official-chromium",
        right: "native-chromium",
        mode: ComparisonMode::Canonical,
    },
    Comparison {
        id: "svg-rust",
        format: "svg",
        profile: Some("canonical-svg-pure-rust-v1"),
        left: "official-rust",
        right: "native-rust",
        mode: ComparisonMode::PureRustSvg,
    },
    Comparison {
        id: "pdf-pdfium",
        format: "pdf",
        profile: Some("canonical-pdf-pdfium-v1"),
        left: "official-chromium",
        right: "native-pdfium",
        mode: ComparisonMode::Canonical,
    },
    Comparison {
        id: "pdf-pdfjs",
        format: "pdf",
        profile: Some("canonical-pdf-pdfjs-v1"),
        left: "official-chromium",
        right: "native-pdfjs",
        mode: ComparisonMode::PdfJs,
    },
    Comparison {
        id: "png-canvas",
        format: "png",
        profile: None,
        left: "official-png",
        right: "native.png",
        mode: ComparisonMode::Canonical,
    },
];

pub(super) fn specification(id: &str) -> Result<Comparison> {
    COMPARISONS
        .iter()
        .find(|c| c.id == id)
        .copied()
        .ok_or_else(|| format!("unknown comparison {id}"))
}

fn class(name: &str) -> Result<VisualClass> {
    match name {
        "geometry/image" => Ok(VisualClass::GeometryImage),
        "typography" => Ok(VisualClass::Typography),
        "mixed-representative" => Ok(VisualClass::MixedRepresentative),
        _ => Err(format!("unknown visual class {name}")),
    }
}

fn coordinate(value: &Value) -> Result<u32> {
    value
        .as_u64()
        .and_then(|n| n.try_into().ok())
        .ok_or("invalid region coordinate".into())
}

#[derive(Clone)]
pub(super) struct Measurement {
    pub changed_pixels: u64,
    pub ssim: SsimOutcome,
    pub passed: bool,
}

impl From<&RegionReport> for Measurement {
    fn from(report: &RegionReport) -> Self {
        Self {
            changed_pixels: report.changed_pixels,
            ssim: report.ssim.clone(),
            passed: report.passed(),
        }
    }
}

impl Measurement {
    pub fn score(&self) -> Result<f64> {
        match self.ssim {
            SsimOutcome::Score(score) if score.is_finite() => Ok(score),
            _ => Err("comparison has no finite SSIM score".into()),
        }
    }
}

pub(super) fn ssim(bits: &str) -> Result<f64> {
    require(
        bits.len() == 16
            && bits
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)),
        "invalid SSIM bits",
    )?;
    let score = f64::from_bits(u64::from_str_radix(bits, 16).map_err(|e| e.to_string())?);
    require(score.is_finite(), "non-finite SSIM ceiling")?;
    Ok(score)
}

type Regions = BTreeMap<String, Measurement>;

struct Envelope {
    changed: u64,
    score: f64,
    pairs: BTreeMap<String, Value>,
}

pub(super) struct FailingRegion {
    pub id: String,
    pub changed: u64,
    pub score: f64,
    pub control_changed: u64,
    pub control_score: f64,
    pub within: bool,
    controls: BTreeMap<String, Value>,
}

fn tiles(length: u32, size: u32) -> Vec<(u32, u32)> {
    let mut result = Vec::new();
    let mut start = 0;
    while start < length {
        let remaining = length - start;
        let extent = if remaining <= size || remaining - size < 11 {
            remaining
        } else {
            size
        };
        result.push((start, extent));
        start += extent;
    }
    result
}

pub(super) struct Variant {
    directory: std::path::PathBuf,
    width: u32,
    height: u32,
    whole_class: VisualClass,
    crops: Vec<Crop>,
    classes: BTreeMap<String, String>,
    rasters: BTreeMap<String, CanonicalRgba8>,
    controls: BTreeMap<(String, String), Regions>,
}

impl Variant {
    pub fn report(&mut self, spec: Comparison) -> Result<Value> {
        let regions = self.candidate(spec)?;
        let region = |id: &str| -> Result<Value> {
            let measured = regions.get(id).ok_or("missing comparison region")?;
            Ok(
                json!({"changed_pixels":measured.changed_pixels,"passed":measured.passed,
                "ssim_bits":match measured.ssim { SsimOutcome::Score(v) => Some(format!("{:016x}",v.to_bits())),_=>None }}),
            )
        };
        let mut crops = Vec::new();
        for id in self.classes.keys().filter(|id| id.as_str() != "whole") {
            crops.push(json!({"id":id,"region":region(id)?}));
        }
        Ok(
            json!({"schema":"excalidraw-render/representative-comparison/v2","comparison":spec.id,"passed":regions.values().all(|r|r.passed),"whole":region("whole")?,"crops":crops}),
        )
    }

    pub fn load(directory: &Path, tiles: Option<u32>) -> Result<Self> {
        require(
            !fs::symlink_metadata(directory)
                .map_err(|e| e.to_string())?
                .file_type()
                .is_symlink(),
            "symlink viewer variant directory",
        )?;
        let mut plan = decode(&read(directory, "regions.json")?)?;
        let width = coordinate(&plan["width"])?;
        let height = coordinate(&plan["height"])?;
        require(width > 0 && height > 0, "zero raster dimension")?;
        let whole_class = class(text(&plan, "whole_class")?)?;
        if let Some(size) = tiles {
            require(
                size >= 11 && width >= 11 && height >= 11,
                "tiles require at least 11 pixels per side",
            )?;
            let crops = plan["crops"].as_array_mut().ok_or("missing crops")?;
            for (y, h) in self::tiles(height, size) {
                for (x, w) in self::tiles(width, size) {
                    crops.push(json!({"id": format!("tile-{x}-{y}"), "class": "geometry/image", "x": x, "y": y, "width": w, "height": h}));
                }
            }
        }
        let mut classes =
            BTreeMap::from([("whole".into(), text(&plan, "whole_class")?.to_owned())]);
        let mut crops = Vec::new();
        for crop in array(&plan, "crops")? {
            let id = text(crop, "id")?;
            require(
                classes
                    .insert(id.into(), text(crop, "class")?.to_owned())
                    .is_none(),
                "duplicate or reserved region id",
            )?;
            let (x, y, w, h) = (
                coordinate(&crop["x"])?,
                coordinate(&crop["y"])?,
                coordinate(&crop["width"])?,
                coordinate(&crop["height"])?,
            );
            require(
                x.checked_add(w).is_some_and(|end| end <= width)
                    && y.checked_add(h).is_some_and(|end| end <= height),
                "crop outside raster",
            )?;
            crops.push(
                Crop::new(id, x, y, w, h, class(text(crop, "class")?)?)
                    .map_err(|e| e.to_string())?,
            );
        }
        Ok(Self {
            directory: directory.to_owned(),
            width,
            height,
            whole_class,
            crops,
            classes,
            rasters: BTreeMap::new(),
            controls: BTreeMap::new(),
        })
    }

    fn load_raster(&mut self, name: &str) -> Result<()> {
        if self.rasters.contains_key(name) {
            return Ok(());
        }
        let length = (self.width as usize)
            .checked_mul(self.height as usize)
            .and_then(|n| n.checked_mul(4))
            .ok_or("raster size overflow")?;
        let rgba = if name.ends_with(".png") {
            let bytes = read(&self.directory, name)?;
            let mut reader = png::Decoder::new(std::io::Cursor::new(bytes))
                .read_info()
                .map_err(|e| e.to_string())?;
            let info = reader.info();
            require(
                (info.width, info.height, info.color_type, info.bit_depth)
                    == (
                        self.width,
                        self.height,
                        png::ColorType::Rgba,
                        png::BitDepth::Eight,
                    ),
                "PNG dimensions or RGBA8 layout differ",
            )?;
            let mut rgba = Vec::new();
            rgba.try_reserve_exact(length).map_err(|e| e.to_string())?;
            rgba.resize(length, 0);
            let frame = reader.next_frame(&mut rgba).map_err(|e| e.to_string())?;
            require(frame.buffer_size() == length, "PNG raster length differs")?;
            rgba
        } else {
            let bytes = read(&self.directory, &format!("{name}.rgba.zlib"))?;
            miniz_oxide::inflate::decompress_to_vec_zlib_with_limit(&bytes, length)
                .map_err(|e| format!("raster inflate: {e:?}"))?
        };
        self.rasters.insert(
            name.into(),
            CanonicalRgba8::new(self.width, self.height, rgba).map_err(|e| e.to_string())?,
        );
        Ok(())
    }

    fn compare(&mut self, left: &str, right: &str, mode: ComparisonMode) -> Result<Regions> {
        self.load_raster(left)?;
        self.load_raster(right)?;
        let report = compare(
            &self.rasters[left],
            &self.rasters[right],
            self.whole_class,
            &self.crops,
            mode,
        )
        .map_err(|e| e.to_string())?;
        let mut regions = BTreeMap::from([("whole".into(), Measurement::from(&report.whole))]);
        for crop in report.crops {
            regions.insert(crop.id, Measurement::from(&crop.region));
        }
        Ok(regions)
    }

    pub fn candidate(&mut self, spec: Comparison) -> Result<Regions> {
        self.compare(spec.left, spec.right, spec.mode)
    }

    fn envelope(&mut self, format: &str) -> Result<BTreeMap<String, Envelope>> {
        let pairs: &[(&str, &str)] = match format {
            "svg" => &[
                ("official-chromium", "official-rust"),
                ("official-chromium", "official-png"),
            ],
            "pdf" => &[
                ("official-chromium", "official-rust"),
                ("official-chromium", "official-png"),
                ("native-pdfium", "native-pdfjs"),
            ],
            "png" => &[
                ("official-png", "official-chromium"),
                ("official-png", "official-rust"),
            ],
            _ => return Err("invalid comparison format".into()),
        };
        let mut envelope: BTreeMap<String, Envelope> = BTreeMap::new();
        for &(left, right) in pairs {
            let key = (left.to_owned(), right.to_owned());
            if !self.controls.contains_key(&key) {
                let result = self.compare(left, right, ComparisonMode::Canonical)?;
                self.controls.insert(key.clone(), result);
            }
            for (id, m) in &self.controls[&key] {
                let score = m.score()?;
                let row = envelope.entry(id.clone()).or_insert_with(|| Envelope {
                    changed: 0,
                    score: f64::INFINITY,
                    pairs: BTreeMap::new(),
                });
                row.changed = row.changed.max(m.changed_pixels);
                row.score = row.score.min(score);
                row.pairs.insert(
                    format!("{left} vs {right}"),
                    json!({"changed_pixels":m.changed_pixels,"ssim":score}),
                );
            }
        }
        Ok(envelope)
    }

    pub fn classify(&mut self, spec: Comparison) -> Result<Vec<FailingRegion>> {
        let measured = self.candidate(spec)?;
        if measured.values().all(|m| m.passed) {
            return Ok(Vec::new());
        }
        let mut envelopes = self.envelope(spec.format)?;
        measured
            .into_iter()
            .filter(|(_, m)| !m.passed)
            .map(|(id, m)| {
                let score = m.score()?;
                let control = envelopes.remove(&id).ok_or("missing control region")?;
                Ok(FailingRegion {
                    id,
                    changed: m.changed_pixels,
                    score,
                    control_changed: control.changed,
                    control_score: control.score,
                    within: m.changed_pixels <= control.changed && score >= control.score,
                    controls: control.pairs,
                })
            })
            .collect()
    }
}

pub(super) fn controls(args: ControlsArgs) -> Result<()> {
    let ControlsArgs {
        directory,
        comparisons: ids,
        tiles,
        output,
    } = args;
    let mut comparisons = Vec::new();
    for id in &ids {
        let spec = specification(id)?;
        require(
            !comparisons.iter().any(|c: &Comparison| c.id == spec.id),
            "duplicate comparison",
        )?;
        comparisons.push(spec);
    }
    if comparisons.is_empty() {
        comparisons.extend(COMPARISONS);
    }
    let mut variant = Variant::load(Path::new(&directory), tiles)?;
    let mut rows = Vec::new();
    for spec in comparisons {
        for region in variant.classify(spec)? {
            rows.push(json!({"comparison":spec.id,"region":region.id,"class":variant.classes[&region.id],"changed_pixels":region.changed,"ssim":region.score,
                "control_changed_pixels":region.control_changed,"control_ssim":region.control_score,"controls":region.controls,"verdict":if region.within { "within" } else { "beyond" }}));
        }
    }
    let record = json!({"schema":"excalidraw-render/identical-input-controls/v1","approval":null,"variant":directory,"tiles":tiles,"failing_regions":rows});
    let bytes = encode(&record)?;
    if let Some(path) = output {
        // An observation must never overwrite inputs or an earlier report.
        use std::io::Write;
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .map_err(|e| e.to_string())?;
        file.write_all(&bytes).map_err(|e| e.to_string())?;
    }
    print!("{}", String::from_utf8(bytes).map_err(|e| e.to_string())?);
    Ok(())
}
