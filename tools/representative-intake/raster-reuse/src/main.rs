//! Coverage-only experiment: retain tiny-skia stroke expansion; compare Zeno
//! alpha against independently captured pinned browser samples, never approve.
use serde_json::{json, Value};
use std::{env, fs};
use tiny_skia::{PathBuilder, PathSegment, Stroke};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args().skip(1).collect();
    if args.len() != 2 {
        return Err("usage: probe <canvas-stroke.json> <new-report.json>".into());
    }
    let oracle: Value = serde_json::from_slice(&fs::read(&args[0])?)?;
    let mut rows = Vec::new();
    for row in oracle["rows"].as_array().ok_or("rows")? {
        let phase = row["phase"].as_f64().ok_or("phase")? as f32;
        let kind = row["kind"].as_str().ok_or("kind")?;
        let mut builder = PathBuilder::new();
        match kind {
            "horizontal-line" => {
                builder.move_to(10., 30. + phase);
                builder.line_to(90., 30. + phase);
            }
            "vertical-line" => {
                builder.move_to(30. + phase, 10.);
                builder.line_to(30. + phase, 90.);
            }
            "rounded-rectangle" => {
                let y = 30. + phase;
                builder.move_to(20., y);
                builder.line_to(80., y);
                builder.cubic_to(90., y, 90., y, 90., y + 10.);
                builder.line_to(90., y + 40.);
                builder.cubic_to(90., y + 50., 90., y + 50., 80., y + 50.);
                builder.line_to(20., y + 50.);
                builder.cubic_to(10., y + 50., 10., y + 50., 10., y + 40.);
                builder.line_to(10., y + 10.);
                builder.cubic_to(10., y, 10., y, 20., y);
                builder.close();
            }
            _ => return Err("unknown shape".into()),
        }
        let path = builder
            .finish()
            .ok_or("path")?
            .stroke(
                &Stroke {
                    width: 2.,
                    ..Stroke::default()
                },
                4.,
            )
            .ok_or("stroke")?;
        let commands: Vec<_> = path
            .segments()
            .map(|segment| {
                let p = |p: tiny_skia::Point| zeno::Point::new(p.x, p.y);
                match segment {
                    PathSegment::MoveTo(a) => zeno::Command::MoveTo(p(a)),
                    PathSegment::LineTo(a) => zeno::Command::LineTo(p(a)),
                    PathSegment::QuadTo(a, b) => zeno::Command::QuadTo(p(a), p(b)),
                    PathSegment::CubicTo(a, b, c) => zeno::Command::CurveTo(p(a), p(b), p(c)),
                    PathSegment::Close => zeno::Command::Close,
                }
            })
            .collect();
        let mut mask = vec![0u8; 100 * 100];
        zeno::Mask::new(commands.as_slice())
            .style(zeno::Fill::NonZero)
            .size(100, 100)
            .render_into(&mut mask, None);
        let alpha: Vec<_> = (28..=32)
            .map(|p| {
                mask[if kind == "vertical-line" {
                    50 * 100 + p
                } else {
                    p * 100 + 50
                }]
            })
            .collect();
        let reference: Vec<_> = row["alpha"]
            .as_array()
            .ok_or("alpha")?
            .iter()
            .map(|v| v.as_u64().unwrap() as u8)
            .collect();
        rows.push(
            json!({"kind":kind,"phase":row["phase"],"reference":reference,"zeno":alpha,
            "within_one_alpha":alpha.iter().zip(&reference).all(|(a,b)|a.abs_diff(*b)<=1)}),
        );
    }
    let passed = rows
        .iter()
        .filter(|r| r["within_one_alpha"] == true)
        .count();
    let report = json!({"schema":"excalidraw-render/png-raster-reuse-probe/v1","approval":null,"zeno":"0.3.3",
        "stroke_expansion":"tiny-skia-path 0.11.4, resolution scale 4", "passed":passed,"total":rows.len(),"rows":rows});
    use std::io::Write;
    fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&args[1])?
        .write_all(&serde_json::to_vec_pretty(&report)?)?;
    println!(
        "{passed}/{} controls within one alpha unit",
        report["total"]
    );
    Ok(())
}
