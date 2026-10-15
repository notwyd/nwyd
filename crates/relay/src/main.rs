#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

mod app;
mod theme;

use eframe::egui;

fn main() -> eframe::Result {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .with_ansi(false)
        .try_init();

    eframe::run_native(
        "NWYD Relay",
        eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default()
                .with_title("NWYD Relay")
                .with_inner_size([600.0, 780.0])
                .with_min_inner_size([420.0, 480.0]),
            renderer: eframe::Renderer::Glow,
            ..Default::default()
        },
        Box::new(|creation| {
            creation
                .egui_ctx
                .send_viewport_cmd(egui::ViewportCommand::SetTheme(egui::SystemTheme::Dark));
            Ok(Box::new(app::RelayApp::new(&creation.egui_ctx)))
        }),
    )
}
