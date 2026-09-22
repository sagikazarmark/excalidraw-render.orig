use base64::{engine::general_purpose::STANDARD, Engine as _};
use serde_json::{json, Value};

use super::Case;

pub(super) const SEED: u64 = 0xa54f_f53a_5f1d_36f1;
pub(super) const FAMILIES: usize = 9;

pub(super) struct Generator(u64);

impl Generator {
    pub(super) fn new() -> Self {
        Self(SEED)
    }

    fn next(&mut self) -> usize {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 as u32) as usize
    }

    pub(super) fn case(&mut self, index: usize) -> Case {
        let family = index % FAMILIES;
        if family == 8 {
            // Cross-stage interactions: text, geometry, clipping, local loss
            // and a PDF effect all participate in the same atomic outcome.
            let mut combined = json!({"type":"excalidraw","version":2,
                "elements":[],"appState":{},"files":{}});
            for part in [0, 1, 2, 3, 5, 7] {
                let case = self.case(part);
                let mut scene: Value =
                    serde_json::from_slice(&STANDARD.decode(case.scene_base64).unwrap()).unwrap();
                for element in scene["elements"].as_array_mut().unwrap() {
                    // Keep references intact within each component, including
                    // deliberate duplicate IDs in the repair component.
                    for field in ["id", "frameId"] {
                        if let Some(id) = element[field].as_str() {
                            element[field] = json!(format!("p{part}-{id}"));
                        }
                    }
                    for field in ["startBinding", "endBinding"] {
                        if let Some(id) = element[field]["elementId"].as_str() {
                            element[field]["elementId"] = json!(format!("p{part}-{id}"));
                        }
                    }
                    if let Some(bound) = element["boundElements"].as_array_mut() {
                        for reference in bound {
                            reference["id"] =
                                json!(format!("p{part}-{}", reference["id"].as_str().unwrap()));
                        }
                    }
                    combined["elements"]
                        .as_array_mut()
                        .unwrap()
                        .push(element.clone());
                }
                combined["files"]
                    .as_object_mut()
                    .unwrap()
                    .extend(scene["files"].as_object().unwrap().clone());
            }
            let mut case = from_scene(
                &format!("seed={SEED:016x} case={index} family=mixed"),
                combined,
            );
            case.options = self.next() as u32;
            return case;
        }
        let mut elements = Vec::new();
        let mut files = json!({});
        for n in 0..1 + self.next() % 5 {
            let kind = match family {
                0 => ["rectangle", "ellipse", "diamond"][self.next() % 3],
                1 => ["line", "arrow", "freedraw"][self.next() % 3],
                2 => "text",
                3 => "rectangle",
                4 | 5 => "image",
                6 => ["future", "embeddable", "iframe"][self.next() % 3],
                _ => "rectangle",
            };
            let mut element = json!({"id":format!("e{n}"),"type":kind,
                "x":self.next() % 48,"y":self.next() % 48,"width":24,"height":16,
                "seed":1 + self.next() % 1000,"updated":1,"angle":0,
                "strokeColor":"#1971c2","backgroundColor":"#a5d8ff",
                "fillStyle":(["solid","hachure","cross-hatch"][self.next()%3]),
                "strokeStyle":(["solid","dashed","dotted"][self.next()%3]),
                "strokeWidth":1,"roughness":self.next()%3,"opacity":50 + self.next()%51});
            match family {
                0 => {
                    element["angle"] = json!([0.0, 0.25, -0.5][self.next() % 3]);
                    element["roundness"] = json!({"type":3});
                    element["link"] = json!(
                        ["https://example.com", "./local", "javascript:alert(1)"][self.next() % 3]
                    );
                }
                1 => {
                    element["points"] = json!([[0, 0], [12, 8], [24, 16]]);
                    element["pressures"] = json!([0.2, 0.8, 0.4]);
                    element["simulatePressure"] = json!(false);
                    if kind == "arrow" {
                        element["endArrowhead"] =
                            json!(["arrow", "triangle", "dot"][self.next() % 3]);
                        element["startBinding"] = json!({"elementId":"e0","focus":0,"gap":1});
                    }
                }
                2 => {
                    let text =
                        ["abc", "a\r\nb", "שלום A", "x\u{200d}y", "🙂", "中"][self.next() % 6];
                    element["text"] = json!(text);
                    element["originalText"] = json!(text);
                    element["fontFamily"] = json!([1, 2, 3, 5][self.next() % 4]);
                    element["fontSize"] = json!(12);
                    element["lineHeight"] = json!(1.25);
                    element["textAlign"] = json!(["left", "center", "right"][self.next() % 3]);
                }
                3 => {
                    element["frameId"] = json!("frame");
                    if n == 0 {
                        element["boundElements"] = json!([{"id":"e1","type":"text"}]);
                    }
                }
                4 | 5 => {
                    element["fileId"] = json!("asset");
                    element["status"] = json!("saved");
                    element["scale"] = json!([if n % 2 == 0 { 1 } else { -1 }, 1]);
                }
                7 => {
                    // Admission succeeds; normalization, local recovery and
                    // source-pointer escaping are reached before projection.
                    element["id"] = json!("duplicate");
                    element["seed"] = json!(0);
                    element.as_object_mut().unwrap().remove("updated");
                    element["future/~"] = json!(true);
                    if n == 0 {
                        element["strokeColor"] = json!("not-a-color");
                    }
                }
                _ => {}
            }
            elements.push(element);
        }
        if family == 3 {
            elements.insert(
                0,
                json!({"id":"frame","type":"frame","x":0,"y":0,
                "width":64,"height":64,"name":"F","updated":1}),
            );
        }
        if family == 4 || family == 5 {
            let svg = if family == 5 {
                r#"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16"><defs><filter id="f"><feGaussianBlur stdDeviation="1"/></filter></defs><rect x="3" y="3" width="10" height="10" fill="red" filter="url(#f)"/></svg>"#
            } else {
                r#"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16"><image href="https://fuzz.invalid/image" width="16" height="16"/><script>alert(1)</script><rect width="8" height="8" fill="blue"/></svg>"#
            };
            files["asset"] = json!({"mimeType":"image/svg+xml",
                "dataURL":format!("data:image/svg+xml;base64,{}",STANDARD.encode(svg))});
        }
        let scene = json!({"type":"excalidraw","version":2,"elements":elements,
            "appState":{"viewBackgroundColor":"#ffffff"},"files":files});
        Case {
            id: format!("seed={SEED:016x} case={index} family={family}"),
            scene_base64: STANDARD.encode(serde_json::to_vec(&scene).unwrap()),
            options: self.next() as u32,
            limit_mode: 0,
            output_ceilings: None,
        }
    }

    pub(super) fn mutate(&mut self, case: &mut Case, index: usize) {
        let bytes = STANDARD.decode(&case.scene_base64).unwrap();
        let mut scene: Value = serde_json::from_slice(&bytes).unwrap();
        let mut bytes = match index % 8 {
            0 => bytes[..self.next() % bytes.len()].to_vec(),
            1 => {
                let mut bytes = bytes;
                bytes.push(0xff);
                bytes
            }
            strategy => {
                let elements = scene["elements"].as_array_mut().unwrap();
                let selected = self.next() % elements.len();
                let element = &mut elements[selected];
                match strategy {
                    2 => {
                        element["x"] = json!(1e100);
                        element["height"] = json!(-1);
                    }
                    3 => {
                        element["points"] = json!([[0, 0], [1e100, 0], null]);
                        element["text"] = json!(false);
                    }
                    4 => {
                        element["id"] = Value::Null;
                        element["type"] = json!("future/~");
                    }
                    5 => {
                        element["frameId"] = element["id"].clone();
                    }
                    6 => {
                        element["opacity"] = json!(-1);
                        element["width"] = json!("NaN");
                    }
                    _ => {
                        element["isDeleted"] = json!(true);
                        element["fontSize"] = json!(1e100);
                    }
                }
                serde_json::to_vec(&scene).unwrap()
            }
        };
        bytes.truncate(super::MAX_INPUT);
        case.scene_base64 = STANDARD.encode(bytes);
        case.id.push_str(&format!(" mutation={}", index % 8));
    }
}

pub(super) fn from_scene(id: &str, scene: Value) -> Case {
    Case {
        id: id.into(),
        scene_base64: STANDARD.encode(serde_json::to_vec(&scene).unwrap()),
        options: 7,
        limit_mode: 0,
        output_ceilings: None,
    }
}
