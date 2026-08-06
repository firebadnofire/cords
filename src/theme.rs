use eframe::egui::{self, Color32, CornerRadius, FontFamily, FontId, Stroke, TextStyle, Visuals};

pub const CANVAS: Color32 = Color32::from_rgb(31, 33, 40);
pub const SIDEBAR: Color32 = Color32::from_rgb(43, 45, 53);
pub const RAIL: Color32 = Color32::from_rgb(29, 31, 37);
pub const SURFACE: Color32 = Color32::from_rgb(49, 51, 60);
pub const SURFACE_HOVER: Color32 = Color32::from_rgb(56, 58, 68);
pub const INPUT: Color32 = Color32::from_rgb(35, 37, 44);
pub const BORDER: Color32 = Color32::from_rgb(62, 64, 73);
pub const TEXT: Color32 = Color32::from_rgb(242, 243, 245);
pub const MUTED: Color32 = Color32::from_rgb(175, 177, 184);
pub const FAINT: Color32 = Color32::from_rgb(128, 130, 139);
pub const ACCENT: Color32 = Color32::from_rgb(112, 132, 255);
pub const GREEN: Color32 = Color32::from_rgb(64, 191, 122);
pub const RED: Color32 = Color32::from_rgb(237, 66, 69);

pub fn apply(ctx: &egui::Context) {
    let mut style = (*ctx.style()).clone();
    style.spacing.item_spacing = egui::vec2(8.0, 8.0);
    style.spacing.button_padding = egui::vec2(12.0, 7.0);
    style.spacing.interact_size.y = 38.0;
    style.visuals = Visuals::dark();
    style.visuals.panel_fill = CANVAS;
    style.visuals.window_fill = SURFACE;
    style.visuals.extreme_bg_color = INPUT;
    style.visuals.faint_bg_color = SURFACE;
    style.visuals.widgets.inactive.bg_fill = SURFACE;
    style.visuals.widgets.inactive.weak_bg_fill = SURFACE;
    style.visuals.widgets.hovered.bg_fill = SURFACE_HOVER;
    style.visuals.widgets.active.bg_fill = ACCENT;
    style.visuals.widgets.open.bg_fill = SURFACE_HOVER;
    style.visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, BORDER);
    style.visuals.selection.bg_fill = ACCENT.linear_multiply(0.7);
    style.visuals.selection.stroke = Stroke::new(1.0_f32, TEXT);
    style.visuals.window_corner_radius = CornerRadius::same(12);
    style.visuals.menu_corner_radius = CornerRadius::same(10);
    style.visuals.widgets.inactive.corner_radius = CornerRadius::same(8);
    style.visuals.widgets.hovered.corner_radius = CornerRadius::same(8);
    style.visuals.widgets.active.corner_radius = CornerRadius::same(8);
    style.text_styles.insert(
        TextStyle::Heading,
        FontId::new(22.0, FontFamily::Proportional),
    );
    style
        .text_styles
        .insert(TextStyle::Body, FontId::new(15.0, FontFamily::Proportional));
    style.text_styles.insert(
        TextStyle::Button,
        FontId::new(14.0, FontFamily::Proportional),
    );
    style.text_styles.insert(
        TextStyle::Small,
        FontId::new(12.0, FontFamily::Proportional),
    );
    ctx.set_style(style);
}
