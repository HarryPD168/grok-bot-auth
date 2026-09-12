//! Loading motion for the native shell.
//! Each widget maps to a real wait. Tokens match the HWW teal chrome.

use eframe::egui::{
    self, Align2, Color32, FontId, Pos2, Rect, Sense, Spinner, Stroke, StrokeKind, Ui, Vec2,
};

use crate::theme::{self, Glyph, ACCENT, ACCENT_INK, HOVER, LINE, MUTED, PANEL, TEXT};

pub const TRACK: Color32 = Color32::from_rgb(29, 42, 56);
pub const SHIMMER: Color32 = Color32::from_rgba_premultiplied(28, 28, 28, 28);
pub const VEIL: Color32 = Color32::from_rgba_premultiplied(7, 9, 12, 168);
pub const SHIMMER_PERIOD: f64 = 1.35;
pub const BAR_PERIOD: f64 = 1.2;
pub const GAUGE_SWEEP: f32 = std::f32::consts::PI * 1.5;

#[derive(Clone, Debug, Default)]
pub struct Busy {
    pub models: bool,
    pub chat: bool,
    pub login: bool,
    pub provider: bool,
    pub cursor: bool,
}

impl Busy {
    pub fn any(&self) -> bool {
        self.models || self.chat || self.login || self.provider || self.cursor
    }
}

pub fn shimmer_x(time: f64, width: f32) -> f32 {
    let t = (time / SHIMMER_PERIOD).rem_euclid(1.0) as f32;
    let ease = t * t * (3.0 - 2.0 * t);
    ease * (width + 56.0) - 28.0
}

pub fn bar_chunk(time: f64, width: f32) -> (f32, f32) {
    let t = (time / BAR_PERIOD).rem_euclid(1.0) as f32;
    let chunk = (width * 0.28).max(24.0);
    let x = t * (width + chunk) - chunk;
    (x, chunk)
}

pub fn gauge_sweep(t01: f32) -> f32 {
    t01.clamp(0.0, 1.0) * GAUGE_SWEEP
}

fn stroke_arc(
    painter: &egui::Painter,
    center: Pos2,
    radius: f32,
    start: f32,
    sweep: f32,
    stroke: Stroke,
) {
    if radius <= 0.5 || sweep.abs() < 0.01 {
        return;
    }
    let n = ((sweep.abs() * radius).max(10.0) as usize).clamp(8, 64);
    let mut points = Vec::with_capacity(n + 1);
    for i in 0..=n {
        let a = start + sweep * (i as f32 / n as f32);
        points.push(center + Vec2::angled(a) * radius);
    }
    painter.add(egui::Shape::line(points, stroke));
}

pub fn action_button(
    ui: &mut Ui,
    glyph: Glyph,
    label: &str,
    primary: bool,
    loading: bool,
) -> egui::Response {
    let galley = ui.fonts(|f| f.layout_no_wrap(label.to_owned(), FontId::proportional(14.0), TEXT));
    let size = Vec2::new(28.0 + galley.size().x + 16.0, 32.0);
    let sense = if loading { Sense::hover() } else { Sense::click() };
    let response = ui.allocate_response(size, sense);
    let rect = response.rect;
    let fill = if primary {
        if !loading && response.hovered() {
            Color32::from_rgb(90, 155, 255)
        } else {
            ACCENT
        }
    } else if !loading && response.hovered() {
        HOVER
    } else {
        PANEL
    };
    let ink = if primary { ACCENT_INK } else { TEXT };
    ui.painter().rect_filled(rect, 3.0, fill);
    if !primary {
        ui.painter()
            .rect_stroke(rect, 3.0, Stroke::new(1.0, LINE), StrokeKind::Middle);
    }
    let icon_rect = Rect::from_center_size(
        Pos2::new(rect.left() + 16.0, rect.center().y),
        Vec2::splat(14.0),
    );
    if loading {
        Spinner::new()
            .size(14.0)
            .color(if primary { ACCENT_INK } else { ACCENT })
            .paint_at(ui, icon_rect);
    } else {
        theme::paint_glyph(ui, icon_rect, glyph, ink);
    }
    ui.painter().text(
        Pos2::new(rect.left() + 28.0, rect.center().y),
        Align2::LEFT_CENTER,
        label,
        FontId::proportional(14.0),
        ink,
    );
    response
}

pub fn thin_bar(ui: &mut Ui, time: f64) {
    let width = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, 3.0), Sense::hover());
    ui.painter().rect_filled(rect, 2.0, TRACK);
    let (x, chunk) = bar_chunk(time, rect.width());
    let left = (rect.left() + x).clamp(rect.left(), rect.right());
    let right = (rect.left() + x + chunk).clamp(rect.left(), rect.right());
    if right > left {
        ui.painter().rect_filled(
            Rect::from_min_max(
                Pos2::new(left, rect.top()),
                Pos2::new(right, rect.bottom()),
            ),
            2.0,
            ACCENT,
        );
    }
}

pub fn skeleton_card(ui: &mut Ui, width: f32, time: f64) {
    let response = ui.allocate_response(Vec2::new(width, 44.0), Sense::hover());
    let rect = response.rect;
    ui.painter().rect_filled(rect, 10.0, PANEL);
    ui.painter()
        .rect_stroke(rect, 10.0, Stroke::new(1.0, LINE), StrokeKind::Middle);
    ui.painter().rect_filled(
        Rect::from_min_size(rect.left_top() + Vec2::new(12.0, 16.0), Vec2::new(width * 0.42, 11.0)),
        4.0,
        HOVER,
    );
    ui.painter().rect_filled(
        Rect::from_min_size(
            Pos2::new(rect.right() - 50.0, rect.top() + 13.0),
            Vec2::new(36.0, 18.0),
        ),
        9.0,
        HOVER,
    );
    let clip = ui.painter().with_clip_rect(rect);
    let x = shimmer_x(time, rect.width());
    clip.rect_filled(
        Rect::from_min_size(
            Pos2::new(rect.left() + x, rect.top()),
            Vec2::new(56.0, rect.height()),
        ),
        0.0,
        SHIMMER,
    );
}

pub fn skeleton_grid(ui: &mut Ui, time: f64) {
    let cols = theme::model_grid_cols(ui.available_width());
    let gap = 8.0;
    let card_w = ((ui.available_width() - gap * (cols.saturating_sub(1) as f32)) / cols as f32)
        .floor()
        .max(160.0);
    for _row in 0..2 {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = gap;
            for _ in 0..cols {
                skeleton_card(ui, card_w, time);
            }
        });
        ui.add_space(gap);
    }
}

/// Horseshoe gauge. `t01` is 可用/目录. `spinning` draws a traveling arc instead of a fill.
pub fn gauge(ui: &mut Ui, t01: f32, spinning: bool, caption: &str, value: &str) {
    ui.horizontal(|ui| {
        let (rect, _) = ui.allocate_exact_size(Vec2::splat(56.0), Sense::hover());
        let c = rect.center();
        let r = 20.0;
        let start = std::f32::consts::PI * 0.75;
        let track = Stroke::new(3.0, TRACK);
        let fill = Stroke::new(3.0, ACCENT);
        stroke_arc(ui.painter(), c, r, start, GAUGE_SWEEP, track);
        if spinning {
            let t = ui.input(|i| i.time);
            let head = start + (t * 2.4).rem_euclid(GAUGE_SWEEP as f64) as f32;
            stroke_arc(ui.painter(), c, r, head, 0.9, fill);
        } else {
            stroke_arc(ui.painter(), c, r, start, gauge_sweep(t01), fill);
        }
        ui.add_space(10.0);
        ui.vertical(|ui| {
            ui.add_space(8.0);
            ui.label(egui::RichText::new(value).size(18.0).color(TEXT));
            ui.label(egui::RichText::new(caption).size(11.0).color(MUTED));
        });
    });
}

pub fn login_veil(ui: &mut Ui, time: f64) {
    let rect = ui.max_rect();
    ui.painter().rect_filled(rect, 0.0, VEIL);
    let panel = Rect::from_center_size(rect.center(), Vec2::new(280.0, 108.0));
    ui.painter().rect_filled(panel, 12.0, PANEL);
    ui.painter()
        .rect_stroke(panel, 12.0, Stroke::new(1.0, LINE), StrokeKind::Middle);
    let spin = Rect::from_center_size(panel.center() - Vec2::new(0.0, 14.0), Vec2::splat(22.0));
    Spinner::new().size(22.0).color(ACCENT).paint_at(ui, spin);
    let _ = time;
    ui.painter().text(
        panel.center() + Vec2::new(0.0, 28.0),
        Align2::CENTER_CENTER,
        "等待 Cursor 登录…",
        FontId::proportional(13.0),
        MUTED,
    );
}

pub fn status_spinner(ui: &mut Ui) {
    ui.add(Spinner::new().size(12.0).color(ACCENT));
}

pub fn think_spinner(ui: &mut Ui) {
    ui.horizontal(|ui| {
        ui.add(Spinner::new().size(16.0).color(ACCENT));
        ui.label(egui::RichText::new("正在生成…").size(13.0).color(MUTED));
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn busy_any_is_false_when_idle() {
        assert!(!Busy::default().any());
        assert!(Busy {
            models: true,
            ..Busy::default()
        }
        .any());
    }

    #[test]
    fn shimmer_x_stays_near_card() {
        for i in 0..20 {
            let x = shimmer_x(i as f64 * 0.07, 220.0);
            assert!(x > -40.0 && x < 280.0, "x={x}");
        }
    }

    #[test]
    fn gauge_sweep_ends() {
        assert!((gauge_sweep(0.0) - 0.0).abs() < 1e-5);
        assert!((gauge_sweep(1.0) - GAUGE_SWEEP).abs() < 1e-4);
        assert!((gauge_sweep(0.5) - GAUGE_SWEEP * 0.5).abs() < 1e-4);
    }

    #[test]
    fn bar_chunk_fits_track() {
        let (x, w) = bar_chunk(0.0, 200.0);
        assert!(w >= 24.0);
        assert!(x <= 200.0);
    }
}
