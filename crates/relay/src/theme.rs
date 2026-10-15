use eframe::egui::{self, Color32, FontId, Stroke, TextStyle};

pub const BACKGROUND: Color32 = Color32::from_rgb(16, 18, 17);
pub const PANEL: Color32 = Color32::from_rgb(20, 22, 21);
pub const CONTROL: Color32 = Color32::from_rgb(26, 29, 27);
pub const LINE: Color32 = Color32::from_rgb(41, 47, 43);
pub const TEXT: Color32 = Color32::from_rgb(231, 237, 233);
pub const MUTED: Color32 = Color32::from_rgb(145, 157, 150);
pub const GREEN: Color32 = Color32::from_rgb(91, 214, 171);
pub const GREEN_DARK: Color32 = Color32::from_rgb(17, 43, 34);
pub const ERROR: Color32 = Color32::from_rgb(233, 137, 137);

pub fn install(ctx: &egui::Context) {
    let mut style = egui::Style::default();
    let mut visuals = egui::Visuals::dark();
    visuals.panel_fill = BACKGROUND;
    visuals.window_fill = PANEL;
    visuals.extreme_bg_color = BACKGROUND;
    visuals.faint_bg_color = CONTROL;
    visuals.override_text_color = Some(TEXT);
    visuals.weak_text_color = Some(MUTED);
    visuals.hyperlink_color = GREEN;
    visuals.error_fg_color = ERROR;
    visuals.selection.bg_fill = GREEN_DARK;
    visuals.selection.stroke = Stroke::new(1.0_f32, GREEN);
    visuals.window_stroke = Stroke::new(1.0_f32, LINE);
    for widgets in [
        &mut visuals.widgets.noninteractive,
        &mut visuals.widgets.inactive,
        &mut visuals.widgets.hovered,
        &mut visuals.widgets.active,
        &mut visuals.widgets.open,
    ] {
        widgets.bg_fill = CONTROL;
        widgets.weak_bg_fill = CONTROL;
        widgets.bg_stroke = Stroke::new(1.0_f32, LINE);
        widgets.fg_stroke = Stroke::new(1.0_f32, TEXT);
        widgets.corner_radius = egui::CornerRadius::same(5);
    }
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0_f32, GREEN);
    visuals.widgets.active.bg_fill = GREEN_DARK;
    visuals.widgets.active.weak_bg_fill = GREEN_DARK;
    visuals.widgets.active.bg_stroke = Stroke::new(1.0_f32, GREEN);
    style.visuals = visuals;
    style.spacing.item_spacing = egui::vec2(10.0, 9.0);
    style.spacing.button_padding = egui::vec2(14.0, 9.0);
    style.spacing.interact_size.y = 32.0;
    style.animation_time = 0.1;
    for (kind, size) in [
        (TextStyle::Heading, 23.0),
        (TextStyle::Body, 14.0),
        (TextStyle::Button, 14.0),
        (TextStyle::Small, 12.0),
    ] {
        style.text_styles.insert(kind, FontId::proportional(size));
    }
    ctx.set_style_of(egui::Theme::Dark, style);
    ctx.set_theme(egui::ThemePreference::Dark);
}

pub fn card() -> egui::Frame {
    egui::Frame::new()
        .fill(PANEL)
        .stroke(Stroke::new(1.0_f32, LINE))
        .corner_radius(8)
        .inner_margin(20)
}
