//! Chrome aligned with CC Switch / cursor-byok: dark ground, blue primary,
//! top tabs, painted 18px icons.

use eframe::egui::{
    self, Align2, Color32, FontData, FontDefinitions, FontFamily, FontId, Pos2, Rect, Sense,
    Stroke, StrokeKind, Ui, Vec2,
};

pub const BG: Color32 = Color32::from_rgb(20, 20, 20);
pub const NAV: Color32 = Color32::from_rgb(20, 20, 20);
pub const PANEL: Color32 = Color32::from_rgb(31, 31, 31);
pub const FIELD: Color32 = Color32::from_rgb(42, 42, 42);
pub const HOVER: Color32 = Color32::from_rgb(42, 42, 42);
pub const LINE: Color32 = Color32::from_rgb(48, 48, 48);
pub const TEXT: Color32 = Color32::from_rgb(235, 235, 235);
pub const MUTED: Color32 = Color32::from_rgb(160, 160, 160);
pub const DIM: Color32 = Color32::from_rgb(140, 140, 140);
pub const ACCENT: Color32 = Color32::from_rgb(68, 137, 255);
pub const ACCENT_DIM: Color32 = Color32::from_rgb(32, 56, 110);
pub const ACCENT_INK: Color32 = Color32::from_rgb(255, 255, 255);
pub const OK: Color32 = Color32::from_rgb(0, 201, 120);
pub const OK_DIM: Color32 = Color32::from_rgb(20, 48, 36);
pub const DANGER: Color32 = Color32::from_rgb(232, 92, 92);
pub const DANGER_DIM: Color32 = Color32::from_rgb(64, 28, 28);
pub const USER_BUBBLE: Color32 = Color32::from_rgb(32, 56, 110);
pub const BOT_BUBBLE: Color32 = Color32::from_rgb(31, 31, 31);

#[derive(Clone, Copy)]
pub enum Glyph {
    Chat,
    Models,
    Session,
    Providers,
    Pool,
    Send,
    Spark,
    Plug,
    User,
    Bot,
    Search,
    Trash,
    Openai,
    Xai,
    Anthropic,
    Overview,
    Settings,
    Skill,
    Mcp,
}

pub fn apply(ctx: &egui::Context) {
    ctx.options_mut(|options| {
        options.theme_preference = egui::ThemePreference::Dark;
    });
    let mut visuals = egui::Visuals::dark();
    visuals.panel_fill = BG;
    visuals.window_fill = PANEL;
    visuals.extreme_bg_color = FIELD;
    visuals.faint_bg_color = HOVER;
    visuals.override_text_color = Some(TEXT);
    visuals.widgets.noninteractive.bg_fill = PANEL;
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, MUTED);
    visuals.widgets.noninteractive.corner_radius = 3.0.into();
    visuals.widgets.inactive.bg_fill = HOVER;
    visuals.widgets.inactive.weak_bg_fill = HOVER;
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, TEXT);
    visuals.widgets.inactive.corner_radius = 3.0.into();
    visuals.widgets.hovered.bg_fill = Color32::from_rgb(23, 34, 45);
    visuals.widgets.hovered.weak_bg_fill = Color32::from_rgb(23, 34, 45);
    visuals.widgets.hovered.fg_stroke = Stroke::new(1.0, TEXT);
    visuals.widgets.hovered.corner_radius = 3.0.into();
    visuals.widgets.active.bg_fill = ACCENT_DIM;
    visuals.widgets.active.fg_stroke = Stroke::new(1.0, ACCENT);
    visuals.widgets.active.corner_radius = 3.0.into();
    visuals.selection.bg_fill = Color32::from_rgb(68, 137, 255);
    visuals.selection.stroke = Stroke::new(1.0, TEXT);
    visuals.hyperlink_color = ACCENT;
    visuals.window_corner_radius = 10.0.into();
    visuals.menu_corner_radius = 6.0.into();
    ctx.set_visuals(visuals);
}

pub fn install_fonts(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();
    if let Ok(bytes) = std::fs::read(r"C:\Windows\Fonts\segoeui.ttf") {
        if bytes.len() > 1024 {
            fonts
                .font_data
                .insert("segoe".into(), std::sync::Arc::new(FontData::from_owned(bytes)));
            if let Some(proportional) = fonts.families.get_mut(&FontFamily::Proportional) {
                proportional.insert(0, "segoe".to_owned());
            }
        }
    }
    for (path, index) in [
        (r"C:\Windows\Fonts\msyh.ttc", 0_u32),
        (r"C:\Windows\Fonts\Deng.ttf", 0),
        (r"C:\Windows\Fonts\simhei.ttf", 0),
    ] {
        let Ok(bytes) = std::fs::read(path) else {
            continue;
        };
        if bytes.len() < 1024 {
            continue;
        }
        let mut data = FontData::from_owned(bytes);
        data.index = index;
        fonts
            .font_data
            .insert("cjk".into(), std::sync::Arc::new(data));
        if let Some(proportional) = fonts.families.get_mut(&FontFamily::Proportional) {
            proportional.insert(0, "cjk".to_owned());
        }
        if let Some(mono) = fonts.families.get_mut(&FontFamily::Monospace) {
            mono.push("cjk".to_owned());
        }
        break;
    }
    ctx.set_fonts(fonts);
}

pub fn paint_glyph(ui: &Ui, rect: Rect, glyph: Glyph, color: Color32) {
    let painter = ui.painter();
    let stroke = Stroke::new(1.4, color);
    let c = rect.center();
    let s = rect.width().min(rect.height());
    match glyph {
        Glyph::Chat => {
            let bubble =
                Rect::from_center_size(c - Vec2::new(0.0, 1.2), Vec2::new(s * 0.78, s * 0.52));
            painter.rect_stroke(bubble, 3.0, stroke, StrokeKind::Middle);
            painter.line_segment(
                [
                    Pos2::new(bubble.left() + 4.0, bubble.bottom()),
                    Pos2::new(bubble.left() + 1.5, bubble.bottom() + 5.0),
                ],
                stroke,
            );
            painter.line_segment(
                [
                    Pos2::new(bubble.left() + 1.5, bubble.bottom() + 5.0),
                    Pos2::new(bubble.left() + 11.0, bubble.bottom()),
                ],
                stroke,
            );
        }
        Glyph::Models => {
            let a = Rect::from_center_size(c - Vec2::new(2.0, 2.0), Vec2::splat(s * 0.42));
            let b = Rect::from_center_size(c + Vec2::new(2.4, 2.4), Vec2::splat(s * 0.42));
            painter.rect_stroke(b, 2.0, stroke, StrokeKind::Middle);
            painter.rect_stroke(a, 2.0, stroke, StrokeKind::Middle);
        }
        Glyph::Session => {
            painter.circle_stroke(c - Vec2::new(0.0, 3.2), s * 0.18, stroke);
            painter.circle_stroke(c + Vec2::new(0.0, 5.2), s * 0.32, stroke);
        }
        Glyph::Providers => {
            painter.circle_stroke(c - Vec2::new(3.5, 0.0), s * 0.16, stroke);
            painter.line_segment([c - Vec2::new(0.6, 0.0), c + Vec2::new(6.2, 0.0)], stroke);
            painter.line_segment([c + Vec2::new(2.4, 0.0), c + Vec2::new(2.4, 3.4)], stroke);
            painter.line_segment([c + Vec2::new(4.6, 0.0), c + Vec2::new(4.6, 2.4)], stroke);
        }
        Glyph::Pool => {
            for i in 0..3 {
                let y = c.y - 5.0 + i as f32 * 5.0;
                painter.line_segment([Pos2::new(c.x - 6.5, y), Pos2::new(c.x + 6.5, y)], stroke);
            }
            painter.rect_filled(
                Rect::from_min_size(
                    Pos2::new(rect.left() + 2.0, rect.top() + 3.0),
                    Vec2::new(2.2, s - 6.0),
                ),
                1.0,
                color,
            );
        }
        Glyph::Send => {
            let tip = c + Vec2::new(6.0, 0.0);
            painter.line_segment([c - Vec2::new(6.0, 4.5), tip], stroke);
            painter.line_segment([c - Vec2::new(6.0, -4.5), tip], stroke);
            painter.line_segment(
                [c - Vec2::new(6.0, 4.5), c - Vec2::new(1.5, 0.0)],
                stroke,
            );
            painter.line_segment(
                [c - Vec2::new(6.0, -4.5), c - Vec2::new(1.5, 0.0)],
                stroke,
            );
        }
        Glyph::Spark => {
            // Official Grok mark: white squircle, two black oval eyes.
            painter.rect_filled(rect, s * 0.28, Color32::from_rgb(244, 244, 246));
            let eye_w = s * 0.18;
            let eye_h = s * 0.28;
            let eye_y = c.y - s * 0.02;
            let left = Rect::from_center_size(
                Pos2::new(c.x - s * 0.16, eye_y),
                Vec2::new(eye_w, eye_h),
            );
            let right = Rect::from_center_size(
                Pos2::new(c.x + s * 0.16, eye_y),
                Vec2::new(eye_w, eye_h),
            );
            painter.rect_filled(left, eye_w * 0.5, Color32::from_rgb(18, 18, 20));
            painter.rect_filled(right, eye_w * 0.5, Color32::from_rgb(18, 18, 20));
        }
        Glyph::Plug => {
            painter.rect_stroke(
                Rect::from_center_size(c + Vec2::new(1.5, 0.0), Vec2::new(s * 0.42, s * 0.38)),
                2.0,
                stroke,
                StrokeKind::Middle,
            );
            painter.line_segment([c - Vec2::new(6.5, -3.0), c - Vec2::new(1.0, -3.0)], stroke);
            painter.line_segment([c - Vec2::new(6.5, 3.0), c - Vec2::new(1.0, 3.0)], stroke);
        }
        Glyph::User => {
            painter.circle_stroke(c - Vec2::new(0.0, 2.8), s * 0.16, stroke);
            painter.circle_stroke(c + Vec2::new(0.0, 5.0), s * 0.28, stroke);
        }
        Glyph::Bot => {
            painter.rect_stroke(
                Rect::from_center_size(c, Vec2::new(s * 0.62, s * 0.48)),
                3.0,
                stroke,
                StrokeKind::Middle,
            );
            painter.circle_filled(c - Vec2::new(2.8, 0.0), 1.4, color);
            painter.circle_filled(c + Vec2::new(2.8, 0.0), 1.4, color);
        }
        Glyph::Search => {
            painter.circle_stroke(c - Vec2::new(1.6, 1.6), s * 0.22, stroke);
            painter.line_segment([c + Vec2::new(1.4, 1.4), c + Vec2::new(6.0, 6.0)], stroke);
        }
        Glyph::Trash => {
            painter.rect_stroke(
                Rect::from_center_size(c + Vec2::new(0.0, 1.5), Vec2::new(s * 0.46, s * 0.48)),
                1.5,
                stroke,
                StrokeKind::Middle,
            );
            painter.line_segment(
                [c - Vec2::new(5.5, -s * 0.22), c + Vec2::new(5.5, -s * 0.22)],
                stroke,
            );
        }
        Glyph::Openai => {
            for i in 0..6 {
                let a = i as f32 * std::f32::consts::TAU / 6.0;
                let p = c + Vec2::new(a.cos(), a.sin()) * s * 0.22;
                painter.circle_stroke(p, s * 0.11, stroke);
            }
        }
        Glyph::Xai => {
            let r = s * 0.28;
            painter.line_segment([c + Vec2::new(-r, -r), c + Vec2::new(r, r)], stroke);
            painter.line_segment([c + Vec2::new(-r, r), c + Vec2::new(r, -r)], stroke);
        }
        Glyph::Anthropic => {
            let r = s * 0.30;
            painter.line_segment([c + Vec2::new(0.0, -r), c + Vec2::new(0.0, r)], stroke);
            painter.line_segment(
                [c + Vec2::new(-r * 0.86, -r * 0.5), c + Vec2::new(r * 0.86, r * 0.5)],
                stroke,
            );
            painter.line_segment(
                [c + Vec2::new(-r * 0.86, r * 0.5), c + Vec2::new(r * 0.86, -r * 0.5)],
                stroke,
            );
        }
        Glyph::Overview => {
            painter.rect_stroke(
                Rect::from_center_size(c - Vec2::new(3.0, 0.0), Vec2::new(s * 0.22, s * 0.42)),
                1.5,
                stroke,
                StrokeKind::Middle,
            );
            painter.rect_stroke(
                Rect::from_center_size(c + Vec2::new(4.0, 1.5), Vec2::new(s * 0.22, s * 0.28)),
                1.5,
                stroke,
                StrokeKind::Middle,
            );
        }
        Glyph::Settings => {
            painter.circle_stroke(c, s * 0.16, stroke);
            for i in 0..6 {
                let a = i as f32 * std::f32::consts::TAU / 6.0;
                painter.line_segment(
                    [c + Vec2::new(a.cos(), a.sin()) * s * 0.22, c + Vec2::new(a.cos(), a.sin()) * s * 0.34],
                    stroke,
                );
            }
        }
        Glyph::Skill => {
            painter.rect_stroke(
                Rect::from_center_size(c, Vec2::new(s * 0.55, s * 0.62)),
                2.0,
                stroke,
                StrokeKind::Middle,
            );
            painter.line_segment([c - Vec2::new(4.0, 2.0), c + Vec2::new(4.0, 2.0)], stroke);
            painter.line_segment([c - Vec2::new(4.0, 5.0), c + Vec2::new(2.0, 5.0)], stroke);
        }
        Glyph::Mcp => {
            painter.circle_stroke(c - Vec2::new(4.0, 0.0), s * 0.14, stroke);
            painter.circle_stroke(c + Vec2::new(4.0, 0.0), s * 0.14, stroke);
            painter.line_segment([c - Vec2::new(2.2, 0.0), c + Vec2::new(2.2, 0.0)], stroke);
        }
    }
}

pub fn brand_glyph(kind: crate::providers::ProviderKind) -> Glyph {
    match kind {
        crate::providers::ProviderKind::Openai => Glyph::Openai,
        crate::providers::ProviderKind::Xai => Glyph::Xai,
        crate::providers::ProviderKind::Anthropic => Glyph::Anthropic,
        crate::providers::ProviderKind::Gemini => Glyph::Spark,
        crate::providers::ProviderKind::Zhipu => Glyph::Models,
        crate::providers::ProviderKind::Kimi => Glyph::Bot,
        crate::providers::ProviderKind::Deepseek => Glyph::Search,
        crate::providers::ProviderKind::Generic => Glyph::Providers,
    }
}

pub fn model_grid_cols(width: f32) -> usize {
    if width >= 1280.0 {
        5
    } else if width >= 960.0 {
        4
    } else if width >= 700.0 {
        3
    } else {
        2
    }
}

pub fn top_tab(ui: &mut Ui, selected: bool, glyph: Glyph, label: &str) -> egui::Response {
    let galley = ui.fonts(|f| f.layout_no_wrap(label.to_owned(), FontId::proportional(14.0), TEXT));
    let size = Vec2::new(40.0 + galley.size().x, 34.0);
    let response = ui.allocate_response(size, Sense::click());
    let rect = response.rect;
    if selected {
        ui.painter().rect_filled(rect, 8.0, ACCENT);
    } else if response.hovered() {
        ui.painter().rect_filled(rect, 8.0, HOVER);
    }
    let ink = if selected { ACCENT_INK } else { MUTED };
    let icon_rect = Rect::from_center_size(
        Pos2::new(rect.left() + 14.0, rect.center().y),
        Vec2::splat(14.0),
    );
    paint_glyph(ui, icon_rect, glyph, ink);
    ui.painter().text(
        Pos2::new(rect.left() + 26.0, rect.center().y),
        Align2::LEFT_CENTER,
        label,
        FontId::proportional(14.0),
        ink,
    );
    response
}

pub fn nav_button(ui: &mut Ui, selected: bool, glyph: Glyph, label: &str) -> egui::Response {
    let response = ui.allocate_response(Vec2::new(ui.available_width(), 38.0), Sense::click());
    let rect = response.rect;
    let inner = rect.shrink2(Vec2::new(8.0, 1.0));
    if selected {
        ui.painter().rect_filled(inner, 3.0, Color32::from_rgba_unmultiplied(94, 225, 216, 18));
        ui.painter().rect_filled(
            Rect::from_min_size(
                Pos2::new(inner.left(), inner.top() + 8.0),
                Vec2::new(2.0, inner.height() - 16.0),
            ),
            0.0,
            ACCENT,
        );
    } else if response.hovered() {
        ui.painter().rect_filled(inner, 3.0, HOVER);
    }
    let icon = if selected { ACCENT } else if response.hovered() { TEXT } else { DIM };
    let ink = if selected { ACCENT } else if response.hovered() { TEXT } else { DIM };
    let icon_rect = Rect::from_center_size(
        Pos2::new(inner.left() + 22.0, inner.center().y),
        Vec2::splat(18.0),
    );
    paint_glyph(ui, icon_rect, glyph, icon);
    ui.painter().text(
        Pos2::new(inner.left() + 42.0, inner.center().y),
        Align2::LEFT_CENTER,
        label,
        FontId::proportional(14.0),
        ink,
    );
    response
}

pub fn icon_button(ui: &mut Ui, glyph: Glyph, label: &str, primary: bool) -> egui::Response {
    let galley = ui.fonts(|f| f.layout_no_wrap(label.to_owned(), FontId::proportional(14.0), TEXT));
    let size = Vec2::new(28.0 + galley.size().x + 16.0, 32.0);
    let response = ui.allocate_response(size, Sense::click());
    let rect = response.rect;
    let fill = if primary {
        if response.hovered() {
            Color32::from_rgb(90, 155, 255)
        } else {
            ACCENT
        }
    } else if response.hovered() {
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
    paint_glyph(ui, icon_rect, glyph, ink);
    ui.painter().text(
        Pos2::new(rect.left() + 28.0, rect.center().y),
        Align2::LEFT_CENTER,
        label,
        FontId::proportional(14.0),
        ink,
    );
    response
}

pub fn pill(ui: &mut Ui, text: &str, ok: bool) {
    let color = if ok { OK } else { MUTED };
    let fill = if ok { OK_DIM } else { HOVER };
    let galley = ui.fonts(|f| f.layout_no_wrap(text.to_owned(), FontId::proportional(11.0), color));
    let (rect, _) = ui.allocate_exact_size(Vec2::new(galley.size().x + 12.0, 18.0), Sense::hover());
    ui.painter().rect_filled(rect, 3.0, fill);
    ui.painter().rect_stroke(
        rect,
        3.0,
        Stroke::new(
            1.0,
            if ok {
                Color32::from_rgba_unmultiplied(110, 231, 166, 70)
            } else {
                LINE
            },
        ),
        StrokeKind::Middle,
    );
    ui.painter().text(
        rect.center(),
        Align2::CENTER_CENTER,
        text,
        FontId::proportional(11.0),
        color,
    );
}

pub fn empty_hint(ui: &mut Ui, glyph: Glyph, title: &str, body: &str) {
    let width = ui.available_width().max(1.0);
    ui.add_space(28.0);
    ui.allocate_ui_with_layout(
        Vec2::new(width, 120.0),
        egui::Layout::top_down(egui::Align::Center),
        |ui| {
            ui.set_min_width(width);
            let (rect, _) = ui.allocate_exact_size(Vec2::splat(40.0), Sense::hover());
            paint_glyph(ui, rect, glyph, ACCENT);
            ui.add_space(10.0);
            ui.label(egui::RichText::new(title).size(16.0).color(TEXT));
            ui.add_space(4.0);
            ui.label(egui::RichText::new(body).size(13.0).color(MUTED));
        },
    );
}

pub fn page_head(ui: &mut Ui, title: &str, desc: &str) {
    ui.label(egui::RichText::new(title).size(22.0).color(TEXT));
    ui.add_space(4.0);
    ui.label(egui::RichText::new(desc).size(13.0).color(MUTED));
    ui.add_space(16.0);
}

pub fn meta_line(ui: &mut Ui, key: &str, value: &str, ok: bool) {
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new(key)
                .size(10.0)
                .color(Color32::from_rgb(74, 86, 102)),
        );
        ui.label(egui::RichText::new(value).size(12.0).color(if ok { OK } else { DIM }));
    });
}

pub fn display_font() -> FontFamily {
    FontFamily::Proportional
}

pub fn text_chip(ui: &mut Ui, label: &str, selected: bool) -> egui::Response {
    let galley = ui.fonts(|f| f.layout_no_wrap(label.to_owned(), FontId::proportional(13.0), TEXT));
    let size = Vec2::new(galley.size().x + 20.0, 32.0);
    let response = ui.allocate_response(size, Sense::click());
    let rect = response.rect;
    let fill = if selected {
        ACCENT_DIM
    } else if response.hovered() {
        HOVER
    } else {
        PANEL
    };
    let ink = if selected { ACCENT } else { TEXT };
    ui.painter().rect_filled(rect, 6.0, fill);
    ui.painter().rect_stroke(
        rect,
        6.0,
        Stroke::new(1.0, if selected { ACCENT } else { LINE }),
        StrokeKind::Middle,
    );
    ui.painter().text(
        rect.center(),
        Align2::CENTER_CENTER,
        label,
        FontId::proportional(13.0),
        ink,
    );
    response
}

pub fn brand_chip(ui: &mut Ui, glyph: Glyph, label: &str, selected: bool) -> egui::Response {
    let galley = ui.fonts(|f| f.layout_no_wrap(label.to_owned(), FontId::proportional(13.0), TEXT));
    let size = Vec2::new(26.0 + galley.size().x + 14.0, 32.0);
    let response = ui.allocate_response(size, Sense::click());
    let rect = response.rect;
    let fill = if selected { ACCENT_DIM } else if response.hovered() { HOVER } else { PANEL };
    let ink = if selected { ACCENT } else { TEXT };
    ui.painter().rect_filled(rect, 6.0, fill);
    ui.painter().rect_stroke(
        rect,
        6.0,
        Stroke::new(1.0, if selected { ACCENT } else { LINE }),
        StrokeKind::Middle,
    );
    let icon_rect = Rect::from_center_size(
        Pos2::new(rect.left() + 14.0, rect.center().y),
        Vec2::splat(16.0),
    );
    paint_glyph(ui, icon_rect, glyph, ink);
    ui.painter().text(
        Pos2::new(rect.left() + 26.0, rect.center().y),
        Align2::LEFT_CENTER,
        label,
        FontId::proportional(13.0),
        ink,
    );
    response
}

pub struct ModelCardClick {
    pub select: bool,
    pub start: bool,
}

pub fn model_card(
    ui: &mut Ui,
    width: f32,
    name: &str,
    available: bool,
    selected: bool,
    pinned: bool,
) -> ModelCardClick {
    let response = ui.allocate_response(Vec2::new(width, 48.0), Sense::click());
    let rect = response.rect;
    let fill = if pinned { ACCENT_DIM } else { PANEL };
    let stroke = if selected {
        ACCENT
    } else if response.hovered() {
        DIM
    } else {
        LINE
    };
    ui.painter().rect_filled(rect, 10.0, fill);
    ui.painter()
        .rect_stroke(rect, 10.0, Stroke::new(if selected { 1.6 } else { 1.0 }, stroke), StrokeKind::Middle);
    let tag = if available { "可用" } else { "目录" };
    let tag_color = if available { OK } else { MUTED };
    let tag_fill = if available { OK_DIM } else { HOVER };
    let tag_galley = ui.fonts(|f| {
        f.layout_no_wrap(tag.to_owned(), FontId::proportional(11.0), tag_color)
    });
    let tag_w = tag_galley.size().x + 12.0;
    let tag_rect = Rect::from_center_size(
        Pos2::new(rect.right() - 10.0 - tag_w / 2.0, rect.center().y),
        Vec2::new(tag_w, 18.0),
    );
    let switch_rect = Rect::from_center_size(
        Pos2::new(tag_rect.left() - 22.0, rect.center().y),
        Vec2::new(34.0, 18.0),
    );
    ui.painter().rect_filled(
        switch_rect,
        9.0,
        if pinned { ACCENT } else { HOVER },
    );
    let knob_x = if pinned {
        switch_rect.right() - 9.0
    } else {
        switch_rect.left() + 9.0
    };
    ui.painter().circle_filled(
        Pos2::new(knob_x, switch_rect.center().y),
        6.0,
        if pinned { ACCENT_INK } else { MUTED },
    );
    let name_max = (switch_rect.left() - rect.left() - 18.0).max(24.0);
    let mut job = egui::text::LayoutJob::single_section(
        name.to_owned(),
        egui::TextFormat {
            font_id: FontId::proportional(13.0),
            color: TEXT,
            ..Default::default()
        },
    );
    job.wrap.max_width = name_max;
    job.wrap.max_rows = 1;
    job.wrap.break_anywhere = true;
    job.wrap.overflow_character = Some('…');
    let name_galley = ui.fonts(|f| f.layout_job(job));
    ui.painter().galley(
        Pos2::new(
            rect.left() + 12.0,
            rect.center().y - name_galley.size().y / 2.0,
        ),
        name_galley,
        TEXT,
    );
    ui.painter().rect_filled(tag_rect, 9.0, tag_fill);
    ui.painter().galley(
        Pos2::new(
            tag_rect.center().x - tag_galley.size().x / 2.0,
            tag_rect.center().y - tag_galley.size().y / 2.0,
        ),
        tag_galley,
        tag_color,
    );
    let clicked = response.clicked();
    let on_switch = response
        .interact_pointer_pos()
        .is_some_and(|pos| switch_rect.expand(4.0).contains(pos));
    ModelCardClick {
        select: clicked && !on_switch,
        start: clicked && on_switch,
    }
}

pub fn name_fits_card(width: f32, name: &str) -> bool {
    // 可用 tag + 12px pad each side ≈ 52px reserved.
    name.len() <= ((width - 52.0) / 7.0).max(4.0) as usize
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn model_grid_cols_matches_website_four_up() {
        assert_eq!(model_grid_cols(1300.0), 5);
        assert_eq!(model_grid_cols(1100.0), 4);
        assert_eq!(model_grid_cols(700.0), 3);
        assert_eq!(model_grid_cols(400.0), 2);
    }

    #[test]
    fn long_model_name_does_not_fit_narrow_card() {
        assert!(!name_fits_card(
            160.0,
            "Claude Opus 4.6 Extra Long Display Name · Cursor"
        ));
        assert!(name_fits_card(280.0, "Claude Haiku 4.5"));
    }
}
