use eframe::egui::{self, Color32, FontId, Stroke, TextStyle};

pub const BACKGROUND: Color32 = Color32::from_rgb(16, 17, 16);
pub const PANEL: Color32 = Color32::from_rgb(24, 26, 23);
pub const CONTROL: Color32 = Color32::from_rgb(31, 33, 28);
pub const LINE: Color32 = Color32::from_rgb(37, 38, 37);
pub const TEXT: Color32 = Color32::from_rgb(243, 240, 233);
pub const MUTED: Color32 = Color32::from_rgb(176, 178, 166);
pub const GOLD: Color32 = Color32::from_rgb(221, 188, 125);
pub const GOLD_LIGHT: Color32 = Color32::from_rgb(242, 219, 170);
pub const GOLD_DARK: Color32 = Color32::from_rgb(55, 49, 34);
pub const GOLD_LINE: Color32 = Color32::from_rgb(102, 90, 63);
pub const GREEN: Color32 = Color32::from_rgb(169, 191, 149);
pub const ERROR: Color32 = Color32::from_rgb(225, 157, 144);

pub fn install(ctx: &egui::Context) {
    let mut style = egui::Style::default();
    let mut visuals = egui::Visuals::dark();
    visuals.panel_fill = BACKGROUND;
    visuals.window_fill = PANEL;
    visuals.extreme_bg_color = BACKGROUND;
    visuals.faint_bg_color = CONTROL;
    visuals.override_text_color = Some(TEXT);
    visuals.weak_text_color = Some(MUTED);
    visuals.hyperlink_color = GOLD;
    visuals.error_fg_color = ERROR;
    visuals.selection.bg_fill = GOLD_DARK;
    visuals.selection.stroke = Stroke::new(1.0_f32, GOLD_LIGHT);
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
        widgets.corner_radius = egui::CornerRadius::same(3);
    }
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0_f32, GOLD);
    visuals.widgets.active.bg_fill = GOLD_DARK;
    visuals.widgets.active.weak_bg_fill = GOLD_DARK;
    visuals.widgets.active.bg_stroke = Stroke::new(1.0_f32, GOLD);
    visuals.collapsing_header_frame = true;
    style.visuals = visuals;
    style.spacing.item_spacing = egui::vec2(12.0, 10.0);
    style.spacing.button_padding = egui::vec2(16.0, 11.0);
    style.spacing.interact_size.y = 36.0;
    style.animation_time = 0.1;
    for (kind, size) in [
        (TextStyle::Heading, 32.0),
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
        .corner_radius(4)
        .inner_margin(24)
}

pub fn eyebrow(text: &str) -> egui::RichText {
    egui::RichText::new(text)
        .size(10.0)
        .strong()
        .extra_letter_spacing(1.6)
        .color(GOLD)
}

pub fn primary_button(ui: &mut egui::Ui, label: &str) -> egui::Response {
    ui.scope(|ui| {
        let widgets = &mut ui.visuals_mut().widgets;
        widgets.inactive.weak_bg_fill = GOLD;
        widgets.hovered.weak_bg_fill = GOLD_LIGHT;
        widgets.active.weak_bg_fill = GOLD;
        widgets.inactive.bg_stroke = Stroke::new(1.0_f32, GOLD);
        widgets.hovered.bg_stroke = Stroke::new(1.0_f32, GOLD_LIGHT);
        ui.add_sized(
            [ui.available_width(), 46.0],
            egui::Button::new(egui::RichText::new(label).color(BACKGROUND).strong()),
        )
    })
    .inner
}
