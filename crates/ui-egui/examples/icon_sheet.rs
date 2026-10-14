//! Headless icon sheet: every icon in `icons.rs` (or the ones named) at ribbon sizes and enlarged,
//! on light and dark backgrounds, for reviewing the artwork.
//!
//! `cargo run -p wordcraft-ui-egui --example icon_sheet -- out.png [name,name,…]`

use egui::{Color32, FontId, Rect, pos2, vec2};

/// The icon names, read from the `match` arms in `icons.rs`.
fn names() -> Vec<String> {
    let src = include_str!("../src/icons.rs");
    let mut out = Vec::new();
    for line in src.lines() {
        let t = line.trim();
        if !t.starts_with('"') || !t.contains("=>") {
            continue;
        }
        let head = t.split("=>").next().unwrap_or("");
        for part in head.split('|') {
            let n = part.trim().trim_matches('"');
            if !n.is_empty() && n.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.') && !out.iter().any(|o| o == n) {
                out.push(n.to_string());
            }
        }
    }
    out
}

static FONTS: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
const SIZES: [f32; 4] = [16.0, 20.0, 32.0, 64.0];
const CELL_W: f32 = 520.0;
const CELL_H: f32 = 84.0;

fn main() {
    let out = std::env::args().nth(1).expect("usage: icon_sheet out.png [names]");
    let names: Vec<String> = match std::env::args().nth(2) {
        Some(list) => list.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect(),
        None => names(),
    };
    // Pages of 40, so each fits in one GPU texture: out.png, then out-2.png, out-3.png…
    for (page, chunk) in names.chunks(40).enumerate() {
        let path = if page == 0 { out.clone() } else { out.replace(".png", &format!("-{}.png", page + 1)) };
        sheet(chunk.to_vec(), &path);
    }
}

fn sheet(names: Vec<String>, out: &str) {
    FONTS.store(false, std::sync::atomic::Ordering::Relaxed);
    let cols = 2usize;
    let rows = names.len().div_ceil(cols);
    let w = CELL_W * cols as f32;
    let h = (CELL_H * rows as f32).max(CELL_H);
    let mut harness = egui_kittest::Harness::builder().with_size(vec2(w, h)).with_pixels_per_point(2.0).wgpu().build_ui(move |ui| {
        // Some icons draw text in the app's fonts, which load on the next frame.
        if !FONTS.swap(true, std::sync::atomic::Ordering::Relaxed) {
            wordcraft_ui_egui::theme::install_fonts(ui.ctx());
            return;
        }
        let p = ui.painter();
        for (i, name) in names.iter().enumerate() {
            let x0 = (i % cols) as f32 * CELL_W;
            let y0 = (i / cols) as f32 * CELL_H;
            // Light half, then dark half, as the ribbon draws them (line colour, accent).
            for (half, (bg, ink, accent)) in [
                (Color32::from_rgb(0xF8, 0xF8, 0xF8), Color32::from_rgb(0x3B, 0x3B, 0x3B), Color32::from_rgb(0x0F, 0x6C, 0xBD)),
                (Color32::from_rgb(0x29, 0x29, 0x29), Color32::from_rgb(0xE6, 0xE6, 0xE6), Color32::from_rgb(0x4C, 0xA4, 0xF0)),
            ]
            .into_iter()
            .enumerate()
            {
                let hx = x0 + 150.0 + half as f32 * 185.0;
                p.rect_filled(Rect::from_min_size(pos2(hx, y0 + 2.0), vec2(183.0, CELL_H - 4.0)), 4.0, bg);
                let mut x = hx + 8.0;
                for s in SIZES {
                    let r = Rect::from_min_size(pos2(x, y0 + (CELL_H - s) / 2.0), vec2(s, s));
                    wordcraft_ui_egui::icons::paint(p, r, name, ink, accent);
                    x += s + 10.0;
                }
            }
            p.text(pos2(x0 + 6.0, y0 + CELL_H / 2.0), egui::Align2::LEFT_CENTER, name, FontId::monospace(11.0), Color32::from_gray(40));
        }
    });
    harness.run_steps(3);
    harness.render().expect("render").save(out).expect("save png");
    println!("{out}");
}
