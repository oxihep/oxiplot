//! Replays ReX's recorded render history (`tests/history.txt`, converted from
//! upstream's `history_regression_render.yaml` by `scripts/gen_rex_history.py`).
//!
//! Every snippet is laid out with the font it was recorded with and rendered
//! through a backend that records its draw calls. A successful record must give
//! the same size and the same glyphs and rules (in any order, each used once);
//! a failed record must still fail. Values may differ by rounding only.
//!
//! The records are upstream's, except where oxiplot-tex fixed upstream on
//! purpose: the subscripts of 16 records were re-recorded when
//! `subscript_shift_down` stopped reading `subscriptTopMax`. To re-record
//! after such a deliberate change, run with `OXIPLOT_TEX_BLESS=1`: records
//! that differ are rewritten from the current output (look at the diff).

use oxiplot_tex::font::backend::skrifa::SkrifaMathFont;
use oxiplot_tex::font::common::GlyphId;
use oxiplot_tex::layout::{engine::LayoutBuilder, Style};
use oxiplot_tex::{Backend, Cursor, FontBackend, GraphicsBackend, Renderer, RGBA};

/// One draw call: a glyph `(x, y, glyph id, scale)` or a rule `(x, y, w, h)`.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Cmd {
    Symbol(f64, f64, u16, f64),
    Rule(f64, f64, f64, f64),
}

impl Cmd {
    fn parse(line: &str) -> Cmd {
        let f: Vec<&str> = line.split(' ').collect();
        let num = |i: usize| f[i].parse::<f64>().expect("number");
        match f[0] {
            "S" => Cmd::Symbol(num(1), num(2), f[3].parse().expect("glyph id"), num(4)),
            "R" => Cmd::Rule(num(1), num(2), num(3), num(4)),
            other => panic!("unknown command {other:?}"),
        }
    }

    /// The record line for this command.
    fn line(&self) -> String {
        match *self {
            Cmd::Symbol(x, y, g, s) => format!("S {x:?} {y:?} {g} {s:?}"),
            Cmd::Rule(x, y, w, h) => format!("R {x:?} {y:?} {w:?} {h:?}"),
        }
    }

    fn matches(&self, other: &Cmd) -> bool {
        match (*self, *other) {
            (Cmd::Symbol(x, y, g, s), Cmd::Symbol(x2, y2, g2, s2)) => {
                g == g2 && close(x, x2) && close(y, y2) && close(s, s2)
            }
            (Cmd::Rule(x, y, w, h), Cmd::Rule(x2, y2, w2, h2)) => {
                close(x, x2) && close(y, y2) && close(w, w2) && close(h, h2)
            }
            _ => false,
        }
    }
}

/// Equal, or equal up to floating-point rounding.
fn close(a: f64, b: f64) -> bool {
    a == b || (a - b).abs() <= 1e-9 * (1.0 + a.abs())
}

#[derive(Default)]
struct Recorder(Vec<Cmd>);

impl<F> FontBackend<F> for Recorder {
    fn symbol(&mut self, pos: Cursor, gid: GlyphId, scale: f64, _: &F) {
        self.0.push(Cmd::Symbol(pos.x, pos.y, gid.into(), scale));
    }
}

impl GraphicsBackend for Recorder {
    fn rule(&mut self, pos: Cursor, width: f64, height: f64) {
        self.0.push(Cmd::Rule(pos.x, pos.y, width, height));
    }
    fn begin_color(&mut self, _: RGBA) {}
    fn end_color(&mut self) {}
}

impl<F> Backend<F> for Recorder {}

fn unhex(s: &str) -> String {
    let bytes = (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).expect("hex"))
        .collect();
    String::from_utf8(bytes).expect("utf-8 TeX")
}

/// Whether `got` and `want` hold the same commands, each matched once.
fn same_commands(got: &[Cmd], want: &[Cmd]) -> bool {
    if got.len() != want.len() {
        return false;
    }
    let mut used = vec![false; want.len()];
    got.iter().all(|g| {
        let hit = want
            .iter()
            .enumerate()
            .find(|(i, w)| !used[*i] && g.matches(w))
            .map(|(i, _)| i);
        match hit {
            Some(i) => {
                used[i] = true;
                true
            }
            None => false,
        }
    })
}

fn load_font(file: &str) -> SkrifaMathFont<'static> {
    let path = format!("{}/resources/{file}", env!("CARGO_MANIFEST_DIR"));
    let bytes: &'static [u8] = std::fs::read(&path).expect("read font").leak();
    let face = skrifa::FontRef::new(bytes).expect("parse font");
    SkrifaMathFont::from_font(face).expect("font has a MATH table")
}

#[test]
fn renders_match_the_upstream_history() {
    let fonts = [
        ("Xits", load_font("XITS_Math.otf")),
        ("Garamond", load_font("Garamond_Math.otf")),
    ];
    let font = |name: &str| {
        &fonts
            .iter()
            .find(|(n, _)| *n == name)
            .unwrap_or_else(|| panic!("unknown font {name:?}"))
            .1
    };
    let path = format!("{}/tests/history.txt", env!("CARGO_MANIFEST_DIR"));
    let text = std::fs::read_to_string(path).expect("read history");
    let mut lines = text.lines();
    let (mut renders, mut errors) = (0, 0);
    let mut mismatches = Vec::new();
    let bless = std::env::var_os("OXIPLOT_TEX_BLESS").is_some();
    let mut out: Vec<String> = Vec::new();

    while let Some(header) = lines.next() {
        let f: Vec<&str> = header.split(' ').collect();
        let tex = unhex(f[2]);
        let engine = LayoutBuilder::new(font(f[1]))
            .font_size(16.0)
            .style(Style::Display)
            .build();
        let result = oxiplot_tex::parser::parse(&tex)
            .map_err(|_| ())
            .and_then(|nodes| engine.layout(&nodes).map_err(|_| ()));

        match f[0] {
            "ERR" => {
                out.push(header.to_owned());
                errors += 1;
                if result.is_ok() {
                    mismatches.push(format!("{tex}: now renders, was an error"));
                }
            }
            "EQ" => {
                renders += 1;
                let (width, height): (f64, f64) =
                    (f[3].parse().expect("width"), f[4].parse().expect("height"));
                let want: Vec<Cmd> = lines
                    .by_ref()
                    .take_while(|l| *l != "END")
                    .map(Cmd::parse)
                    .collect();
                let Ok(layout) = result else {
                    mismatches.push(format!("{tex}: now fails to lay out"));
                    continue;
                };
                let size = layout.size();
                let mut rec = Recorder::default();
                Renderer::new().render(&layout, &mut rec);
                let before = mismatches.len();
                if !close(size.width, width) || !close(size.height - size.depth, height) {
                    mismatches.push(format!(
                        "{tex}: size {} x {}, want {width} x {height}",
                        size.width,
                        size.height - size.depth
                    ));
                } else if !same_commands(&rec.0, &want) {
                    mismatches.push(format!(
                        "{tex}: {} draw calls differ from the {} recorded",
                        rec.0.len(),
                        want.len()
                    ));
                }
                if mismatches.len() == before {
                    out.push(header.to_owned());
                    out.extend(want.iter().map(Cmd::line));
                } else {
                    out.push(format!(
                        "EQ {} {} {:?} {:?}",
                        f[1],
                        f[2],
                        size.width,
                        size.height - size.depth
                    ));
                    out.extend(rec.0.iter().map(Cmd::line));
                }
                out.push("END".to_owned());
            }
            other => panic!("unknown record {other:?}"),
        }
    }

    assert!(renders > 0, "no records read");
    if bless {
        let path = format!("{}/tests/history.txt", env!("CARGO_MANIFEST_DIR"));
        std::fs::write(path, out.join("\n") + "\n").expect("write history");
        eprintln!("re-recorded {} history records", mismatches.len());
        return;
    }
    for m in &mismatches {
        eprintln!("MISMATCH {m}");
    }
    assert!(
        mismatches.is_empty(),
        "{} of {} history records differ",
        mismatches.len(),
        renders + errors
    );
}
