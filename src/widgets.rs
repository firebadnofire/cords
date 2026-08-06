use crate::theme;
use eframe::egui::{self, Align2, Color32, FontId, Response, Sense, Stroke, Ui, Vec2};

pub fn avatar(
    ui: &mut Ui,
    initials: &str,
    color: Color32,
    size: f32,
    online: Option<bool>,
) -> Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(size), Sense::click());
    let radius = size / 2.0;
    ui.painter().circle_filled(rect.center(), radius, color);
    ui.painter().text(
        rect.center(),
        Align2::CENTER_CENTER,
        initials,
        FontId::proportional((size * 0.31).max(10.0)),
        Color32::WHITE,
    );
    if let Some(is_online) = online {
        let dot = rect.right_bottom() - egui::vec2(size * 0.12, size * 0.12);
        ui.painter().circle_filled(dot, size * 0.16, theme::SIDEBAR);
        ui.painter().circle_filled(
            dot,
            size * 0.11,
            if is_online {
                theme::GREEN
            } else {
                theme::FAINT
            },
        );
    }
    response
}

pub fn pill(ui: &mut Ui, text: impl Into<String>, fill: Color32) -> Response {
    ui.add(
        egui::Button::new(egui::RichText::new(text).size(12.0).color(theme::TEXT))
            .fill(fill)
            .stroke(Stroke::NONE)
            .corner_radius(8.0),
    )
}

pub fn section_label(ui: &mut Ui, text: &str) {
    ui.add_space(7.0);
    ui.label(
        egui::RichText::new(text.to_uppercase())
            .size(11.0)
            .color(theme::FAINT)
            .strong(),
    );
    ui.add_space(3.0);
}
