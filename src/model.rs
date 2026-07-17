use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExcalidrawFile {
    #[serde(default, rename = "type")]
    pub _document_type: String,
    #[serde(default, rename = "version")]
    pub _version: u64,
    #[serde(default)]
    pub elements: Vec<Element>,
    #[serde(default)]
    pub app_state: AppState,
    #[serde(default, rename = "files")]
    pub _files: serde_json::Value,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct AppState {
    pub view_background_color: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Element {
    pub id: String,
    #[serde(rename = "type")]
    pub element_type: String,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub angle: f64,
    pub stroke_color: Option<String>,
    pub background_color: Option<String>,
    pub fill_style: Option<String>,
    pub stroke_width: Option<f64>,
    pub stroke_style: Option<String>,
    pub roughness: Option<f64>,
    pub opacity: Option<f64>,
    pub seed: Option<i64>,
    #[serde(rename = "roundness")]
    pub _roundness: Option<serde_json::Value>,
    pub is_deleted: bool,
    pub points: Vec<[f64; 2]>,
}

impl Element {
    pub fn is_supported(&self) -> bool {
        matches!(
            self.element_type.as_str(),
            "rectangle" | "ellipse" | "diamond" | "line" | "arrow"
        )
    }

    pub fn stroke_color(&self) -> &str {
        self.stroke_color
            .as_deref()
            .filter(|color| !color.is_empty())
            .unwrap_or("#1e1e1e")
    }

    pub fn fill_color(&self) -> Option<&str> {
        self.background_color
            .as_deref()
            .filter(|color| !color.is_empty() && !is_transparent(color))
    }

    pub fn stroke_width(&self) -> f64 {
        self.stroke_width.unwrap_or(1.0).max(0.0)
    }

    pub fn roughness(&self) -> f64 {
        self.roughness.unwrap_or(1.0).max(0.0)
    }

    pub fn opacity(&self) -> f64 {
        self.opacity.unwrap_or(100.0).clamp(0.0, 100.0) / 100.0
    }

    pub fn center(&self) -> (f64, f64) {
        (self.x + self.width / 2.0, self.y + self.height / 2.0)
    }
}

pub fn is_transparent(color: &str) -> bool {
    color.eq_ignore_ascii_case("transparent")
}
