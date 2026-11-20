//! SVG backend: render the [`DrawGroup`] IR to a self-contained SVG string.
//!
//! Text is already reduced to glyph-outline paths by [`crate::text`], so the SVG
//! needs no font references and matches the PNG exactly.

use std::fmt::Write;

use crate::color::Color;
use crate::draw::{DrawCommand, DrawGroup, LineCap, LineJoin, Path, Rect, Seg, Stroke};

fn num(v: f32) -> String {
    // A non-finite coordinate would make the whole document invalid; the
    // artists drop non-finite data, so this is only a last guard (the PDF
    // backend does the same).
    let v = if v.is_finite() { v } else { 0.0 };
    // Compact fixed-point; trim trailing zeros and a trailing dot.
    let s = format!("{v:.3}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s.is_empty() || s == "-0" {
        "0".to_string()
    } else {
        s.to_string()
    }
}

fn hex6(c: Color) -> String {
    format!("#{:02x}{:02x}{:02x}", c.r, c.g, c.b)
}

fn fill_attr(fill: Option<Color>) -> String {
    match fill {
        Some(c) => {
            let mut s = format!(" fill=\"{}\"", hex6(c));
            if c.a < 255 {
                let _ = write!(s, " fill-opacity=\"{}\"", num(c.opacity()));
            }
            s
        }
        None => " fill=\"none\"".to_string(),
    }
}

fn cap_str(c: LineCap) -> &'static str {
    match c {
        LineCap::Butt => "butt",
        LineCap::Round => "round",
        LineCap::Square => "square",
    }
}

fn join_str(j: LineJoin) -> &'static str {
    match j {
        LineJoin::Miter => "miter",
        LineJoin::Round => "round",
        LineJoin::Bevel => "bevel",
    }
}

fn stroke_attr(stroke: Option<&Stroke>) -> String {
    let Some(s) = stroke else {
        return String::new();
    };
    if s.width <= 0.0 || s.color.a == 0 {
        return " stroke=\"none\"".to_string();
    }
    let mut out = format!(
        " stroke=\"{}\" stroke-width=\"{}\" stroke-linecap=\"{}\" stroke-linejoin=\"{}\"",
        hex6(s.color),
        num(s.width),
        cap_str(s.cap),
        join_str(s.join),
    );
    if s.color.a < 255 {
        let _ = write!(out, " stroke-opacity=\"{}\"", num(s.color.opacity()));
    }
    if let Some(dash) = &s.dash {
        let pat: Vec<String> = dash.iter().map(|d| num(*d)).collect();
        let _ = write!(out, " stroke-dasharray=\"{}\"", pat.join(","));
    }
    out
}

fn points_attr(pts: &[(f32, f32)]) -> String {
    pts.iter()
        .map(|(x, y)| format!("{},{}", num(*x), num(*y)))
        .collect::<Vec<_>>()
        .join(" ")
}

fn path_d(path: &Path) -> String {
    let mut d = String::new();
    for seg in &path.segs {
        match *seg {
            Seg::Move(x, y) => {
                let _ = write!(d, "M{} {} ", num(x), num(y));
            }
            Seg::Line(x, y) => {
                let _ = write!(d, "L{} {} ", num(x), num(y));
            }
            Seg::Quad(cx, cy, x, y) => {
                let _ = write!(d, "Q{} {} {} {} ", num(cx), num(cy), num(x), num(y));
            }
            Seg::Cubic(c1x, c1y, c2x, c2y, x, y) => {
                let _ = write!(
                    d,
                    "C{} {} {} {} {} {} ",
                    num(c1x),
                    num(c1y),
                    num(c2x),
                    num(c2y),
                    num(x),
                    num(y)
                );
            }
            Seg::Close => d.push_str("Z "),
        }
    }
    d.trim_end().to_string()
}

fn emit_cmd(out: &mut String, cmd: &DrawCommand) {
    match cmd {
        DrawCommand::Polyline { pts, stroke } => {
            let _ = write!(
                out,
                "<polyline points=\"{}\" fill=\"none\"{}/>",
                points_attr(pts),
                stroke_attr(Some(stroke))
            );
        }
        DrawCommand::Polygon { pts, fill, stroke } => {
            let _ = write!(
                out,
                "<polygon points=\"{}\"{}{}/>",
                points_attr(pts),
                fill_attr(*fill),
                stroke_attr(stroke.as_ref())
            );
        }
        DrawCommand::Line { p0, p1, stroke } => {
            let _ = write!(
                out,
                "<line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\"{}/>",
                num(p0.0),
                num(p0.1),
                num(p1.0),
                num(p1.1),
                stroke_attr(Some(stroke))
            );
        }
        DrawCommand::Rect { rect, fill, stroke } => {
            let _ = write!(
                out,
                "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"{}{}/>",
                num(rect.x),
                num(rect.y),
                num(rect.w),
                num(rect.h),
                fill_attr(*fill),
                stroke_attr(stroke.as_ref())
            );
        }
        DrawCommand::Circle { c, r, fill, stroke } => {
            let _ = write!(
                out,
                "<circle cx=\"{}\" cy=\"{}\" r=\"{}\"{}{}/>",
                num(c.0),
                num(c.1),
                num(*r),
                fill_attr(*fill),
                stroke_attr(stroke.as_ref())
            );
        }
        DrawCommand::Path { path, fill, stroke } => {
            let _ = write!(
                out,
                "<path d=\"{}\"{}{}/>",
                path_d(path),
                fill_attr(*fill),
                stroke_attr(stroke.as_ref())
            );
        }
        DrawCommand::Image {
            rect,
            width,
            height,
            rgba,
        } => emit_image(out, *rect, *width, *height, rgba),
    }
}

/// An image as an embedded PNG, scaled without smoothing so each pixel stays
/// a crisp cell.
#[cfg(feature = "png")]
fn emit_image(out: &mut String, rect: Rect, width: u32, height: u32, rgba: &[u8]) {
    let Some(png) = crate::render::raster::encode_rgba_png(width, height, rgba) else {
        return;
    };
    let _ = write!(
        out,
        "<image x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" preserveAspectRatio=\"none\" \
         style=\"image-rendering:pixelated\" xlink:href=\"data:image/png;base64,{}\"/>",
        num(rect.x),
        num(rect.y),
        num(rect.w),
        num(rect.h),
        base64(&png)
    );
}

/// Without a PNG encoder, an image is drawn as one rectangle per visible
/// pixel (what a heatmap was before it became an image).
#[cfg(not(feature = "png"))]
fn emit_image(out: &mut String, rect: Rect, width: u32, height: u32, rgba: &[u8]) {
    let (cw, ch) = (rect.w / width as f32, rect.h / height as f32);
    for (i, px) in rgba.as_chunks::<4>().0.iter().enumerate() {
        if px[3] == 0 {
            continue;
        }
        // `i` indexes `width × height` pixels, both `u32`.
        let i = u32::try_from(i).unwrap_or(u32::MAX);
        let (col, row) = (i % width, i / width);
        let cell = Rect::new(rect.x + col as f32 * cw, rect.y + row as f32 * ch, cw, ch);
        let fill = Color::rgba(px[0], px[1], px[2], px[3]);
        emit_cmd(
            out,
            &DrawCommand::Rect {
                rect: cell,
                fill: Some(fill),
                stroke: None,
            },
        );
    }
}

/// Standard base64 (RFC 4648, padded).
#[cfg(feature = "png")]
fn base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b = [
            chunk[0],
            *chunk.get(1).unwrap_or(&0),
            *chunk.get(2).unwrap_or(&0),
        ];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        for k in 0..4 {
            if k <= chunk.len() {
                out.push(ALPHABET[(n >> (18 - 6 * k)) as usize & 63] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

/// Render the IR (pixels at `dpi`) to a complete SVG document string.
///
/// The document's size is given in points, the figure's physical size (a
/// 6.4 × 4.8 in figure is `460.8pt × 345.6pt`, as matplotlib writes it), so it
/// opens at the same size as the PDF; the drawing keeps pixel coordinates
/// through the `viewBox`.
#[must_use]
pub fn render(groups: &[DrawGroup], width: u32, height: u32, bg: Color, dpi: f32) -> String {
    let scale = if dpi.is_finite() && dpi > 0.0 {
        72.0 / dpi
    } else {
        1.0
    };
    let (w_pt, h_pt) = (num(width as f32 * scale), num(height as f32 * scale));
    let mut out = String::new();
    let _ = write!(
        out,
        "<svg xmlns=\"http://www.w3.org/2000/svg\" \
         xmlns:xlink=\"http://www.w3.org/1999/xlink\" width=\"{w_pt}pt\" height=\"{h_pt}pt\" \
         viewBox=\"0 0 {width} {height}\">"
    );

    // Clip paths in <defs>, one per group that needs one.
    let clips: Vec<(usize, Rect)> = groups
        .iter()
        .enumerate()
        .filter_map(|(i, g)| g.clip.map(|r| (i, r)))
        .collect();
    if !clips.is_empty() {
        out.push_str("<defs>");
        for (i, r) in &clips {
            let _ = write!(
                out,
                "<clipPath id=\"clip{i}\"><rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"/></clipPath>",
                num(r.x),
                num(r.y),
                num(r.w),
                num(r.h)
            );
        }
        out.push_str("</defs>");
    }

    // Opaque background.
    let _ = write!(
        out,
        "<rect x=\"0\" y=\"0\" width=\"{width}\" height=\"{height}\" fill=\"{}\"/>",
        hex6(bg)
    );

    for (i, group) in groups.iter().enumerate() {
        let clipped = group.clip.is_some();
        if clipped {
            let _ = write!(out, "<g clip-path=\"url(#clip{i})\">");
        }
        for cmd in &group.cmds {
            emit_cmd(&mut out, cmd);
        }
        if clipped {
            out.push_str("</g>");
        }
    }

    out.push_str("</svg>");
    out
}
