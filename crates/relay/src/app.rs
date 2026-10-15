use std::{path::Path, time::Duration};

use eframe::egui::{self, Color32, RichText};
use nwyd_relay::{PLAY_URL, RelayConfig, RelayController, RelaySnapshot, RelayStatus};

use crate::theme;

const EXITLAG_HELP: &str =
    "https://www.exitlag.com/faq/functionalities/my-game-is-not-on-exitlag-how-do-i-add";
const EXECUTABLE: &str = if cfg!(windows) {
    "nwyd-relay.exe"
} else {
    "nwyd-relay"
};

pub struct RelayApp {
    relay: Option<RelayController>,
    executable: String,
}

impl RelayApp {
    pub fn new(ctx: &egui::Context) -> Self {
        theme::install(ctx);
        Self {
            relay: None,
            executable: executable_name(std::env::current_exe().ok().as_deref()),
        }
    }
}

impl eframe::App for RelayApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let snapshot = self.relay.as_ref().map(RelayController::snapshot);
        let mut view = View::from_snapshot(snapshot.as_ref());
        view.executable = &self.executable;
        match draw(ctx, &view) {
            Action::OpenPlayPage => ctx.open_url(egui::OpenUrl::new_tab(PLAY_URL)),
            Action::Retry => {
                if let Some(relay) = &mut self.relay {
                    relay.retry();
                }
            }
            Action::Stop => {
                if let Some(relay) = &mut self.relay {
                    relay.shutdown();
                }
            }
            Action::None => {}
        }
        if self.relay.is_none() {
            self.relay = Some(RelayController::new(RelayConfig::default()));
        }
        ctx.request_repaint_after(Duration::from_millis(250));
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        if let Some(relay) = &mut self.relay {
            relay.shutdown();
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Action {
    None,
    OpenPlayPage,
    Retry,
    Stop,
}

struct View<'a> {
    status: &'a RelayStatus,
    executable: &'a str,
    address: Option<String>,
    sessions: u64,
    sent: u64,
    received: u64,
    last_error: Option<&'a str>,
}

impl<'a> View<'a> {
    fn from_snapshot(snapshot: Option<&'a RelaySnapshot>) -> Self {
        match snapshot {
            Some(snapshot) => Self {
                status: &snapshot.status,
                executable: EXECUTABLE,
                address: snapshot.listen_addr.map(|address| address.to_string()),
                sessions: snapshot.active_sessions,
                sent: snapshot.bytes_to_server,
                received: snapshot.bytes_from_server,
                last_error: snapshot.last_error.as_deref(),
            },
            None => Self {
                status: &RelayStatus::Launching,
                executable: EXECUTABLE,
                address: None,
                sessions: 0,
                sent: 0,
                received: 0,
                last_error: None,
            },
        }
    }

    fn title(&self) -> &'static str {
        match self.status {
            RelayStatus::Launching => "Iniciando relay…",
            RelayStatus::Ready => "Pronto para jogar",
            RelayStatus::Failed(_) => "Não foi possível iniciar o relay",
            RelayStatus::Stopped => "Relay parado",
        }
    }

    fn primary_action(&self) -> Action {
        match self.status {
            RelayStatus::Launching => Action::None,
            RelayStatus::Ready => Action::OpenPlayPage,
            RelayStatus::Failed(_) | RelayStatus::Stopped => Action::Retry,
        }
    }

    fn color(&self) -> Color32 {
        match self.status {
            RelayStatus::Ready => theme::GREEN,
            RelayStatus::Failed(_) => theme::ERROR,
            _ => theme::MUTED,
        }
    }
}

fn draw(ctx: &egui::Context, view: &View<'_>) -> Action {
    let mut action = Action::None;
    egui::TopBottomPanel::top("header")
        .frame(
            egui::Frame::new()
                .fill(theme::BACKGROUND)
                .inner_margin(egui::Margin::symmetric(28, 20)),
        )
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                brand_mark(ui);
                ui.label(
                    RichText::new("NWYD")
                        .size(26.0)
                        .extra_letter_spacing(-1.2)
                        .strong(),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(theme::eyebrow("RELAY"));
                });
            });
            ui.add_space(12.0);
            ui.separator();
        });

    egui::TopBottomPanel::bottom("footer")
        .frame(
            egui::Frame::new()
                .fill(theme::BACKGROUND)
                .inner_margin(egui::Margin::symmetric(28, 14)),
        )
        .show(ctx, |ui| {
            ui.separator();
            ui.add_space(2.0);
            ui.horizontal_wrapped(|ui| {
                ui.label(
                    RichText::new(format!("v{}", env!("CARGO_PKG_VERSION")))
                        .small()
                        .color(theme::GOLD),
                );
                ui.label(
                    RichText::new("Fechar esta janela desconecta suas sessões.")
                        .small()
                        .color(theme::MUTED),
                );
            });
        });

    egui::CentralPanel::default()
        .frame(
            egui::Frame::new()
                .fill(theme::BACKGROUND)
                .inner_margin(egui::Margin::symmetric(28, 4)),
        )
        .show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.add_space(8.0);
                section_label(ui, "JOGUE NO NAVEGADOR");
                ui.add_space(8.0);
                theme::card()
                    .stroke(egui::Stroke::new(1.0_f32, theme::GOLD_LINE))
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.horizontal(|ui| {
                            if matches!(view.status, RelayStatus::Launching) {
                                ui.add(egui::Spinner::new().size(12.0).color(theme::GOLD));
                            } else {
                                status_dot(ui, view.color());
                            }
                            ui.label(theme::eyebrow("CONEXÃO LOCAL").color(view.color()));
                        });
                        ui.add_space(4.0);
                        status_heading(ui, view.title());
                        match view.status {
                            RelayStatus::Launching => {
                                ui.label(RichText::new("Preparando a conexão com o navegador.").color(theme::MUTED));
                            }
                            RelayStatus::Ready => {
                                ui.label(RichText::new("Abra a página do jogo para começar sua sessão.").color(theme::MUTED));
                            }
                            RelayStatus::Failed(error) => {
                                ui.label(RichText::new(error).color(theme::ERROR));
                            }
                            RelayStatus::Stopped => {
                                ui.label(RichText::new("Inicie o relay para conectar o navegador novamente.").color(theme::MUTED));
                            }
                        }
                        ui.add_space(8.0);
                        let primary = view.primary_action();
                        if primary != Action::None {
                            let label = match view.status {
                                RelayStatus::Ready => "Abrir página do jogo",
                                RelayStatus::Stopped => "Iniciar relay",
                                _ => "Tentar novamente",
                            };
                            if theme::primary_button(ui, label).clicked() {
                                action = primary;
                            }
                        }
                        if matches!(view.status, RelayStatus::Ready) {
                            ui.vertical_centered(|ui| {
                                ui.label(RichText::new(PLAY_URL).small().color(theme::MUTED));
                            });
                        }
                        ui.add_space(6.0);
                        ui.separator();
                        ui.horizontal(|ui| {
                            status_dot(ui, view.color());
                            ui.label(RichText::new(session_label(view.sessions)).size(12.0).color(theme::MUTED));
                        });
                        if let Some(address) = &view.address {
                            ui.label(
                                RichText::new(format!("Aguardando conexões em {address}"))
                                    .size(11.0)
                                    .color(theme::MUTED),
                            );
                        }
                    });

                ui.add_space(18.0);
                section_label(ui, "COMO JOGAR");
                ui.add_space(6.0);
                instruction(ui, "01", "Abra a página do jogo e selecione a pasta dos arquivos.");
                instruction(ui, "02", "Permita o acesso à rede local se o navegador solicitar.");
                instruction(ui, "03", "Mantenha o relay aberto enquanto joga.");
                ui.add_space(2.0);
                ui.label(RichText::new("Os arquivos salvos podem ser reutilizados em todas as abas do navegador.").small().color(theme::MUTED));

                ui.add_space(12.0);
                ui.separator();
                egui::CollapsingHeader::new("Usando o ExitLag").show(ui, |ui| {
                    ui.label(format!("Configure o ExitLag para rotear {} antes de iniciar uma sessão. Confirme a conexão no ExitLag.", view.executable));
                    ui.label("Se o NWYD Relay não estiver na lista, solicite suporte para este executável.");
                    ui.hyperlink_to("Solicitar suporte ao ExitLag", EXITLAG_HELP);
                });
                egui::CollapsingHeader::new("Detalhes da conexão").show(ui, |ui| {
                    ui.label(format!("Enviados: {}    Recebidos: {}", format_bytes(view.sent), format_bytes(view.received)));
                    if let Some(error) = view.last_error {
                        ui.label(RichText::new(error).color(theme::ERROR));
                    }
                    if matches!(view.status, RelayStatus::Ready) {
                        let label = if view.sessions > 0 { "Parar relay e desconectar sessões" } else { "Parar relay" };
                        if ui.button(label).clicked() {
                            action = Action::Stop;
                        }
                    }
                });
                ui.add_space(8.0);
            });
        });
    action
}

fn brand_mark(ui: &mut egui::Ui) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(24.0, 30.0), egui::Sense::hover());
    let point = |x, y| rect.min + egui::vec2(x, y);
    let stroke = egui::Stroke::new(1.5_f32, theme::GOLD);
    ui.painter().add(egui::Shape::line(
        vec![
            point(3.0, 25.0),
            point(3.0, 5.0),
            point(21.0, 25.0),
            point(21.0, 5.0),
        ],
        stroke,
    ));
    for (start, end) in [
        (point(3.0, 12.0), point(21.0, 25.0)),
        (point(3.0, 5.0), point(21.0, 18.0)),
    ] {
        ui.painter().line_segment([start, end], stroke);
    }
}

fn section_label(ui: &mut egui::Ui, text: &str) {
    ui.horizontal(|ui| {
        let (rect, _) = ui.allocate_exact_size(egui::vec2(22.0, 12.0), egui::Sense::hover());
        ui.painter()
            .hline(rect.x_range(), rect.center().y, (1.0, theme::GOLD));
        ui.label(theme::eyebrow(text));
    });
}

fn status_heading(ui: &mut egui::Ui, title: &str) {
    let (prefix, accent) = title.rsplit_once(' ').unwrap_or((title, ""));
    let mut job = egui::text::LayoutJob::default();
    for text in [
        RichText::new(format!("{prefix} "))
            .strong()
            .color(theme::TEXT),
        RichText::new(accent).italics().color(theme::GOLD_LIGHT),
    ] {
        text.size(32.0).extra_letter_spacing(-0.8).append_to(
            &mut job,
            ui.style(),
            egui::FontSelection::Default,
            egui::Align::BOTTOM,
        );
    }
    ui.add(egui::Label::new(job).wrap());
}

fn status_dot(ui: &mut egui::Ui, color: Color32) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(8.0, 14.0), egui::Sense::hover());
    ui.painter().circle_filled(rect.center(), 3.0, color);
}

fn instruction(ui: &mut egui::Ui, number: &str, text: &str) {
    ui.horizontal_top(|ui| {
        ui.label(
            RichText::new(number)
                .monospace()
                .size(12.0)
                .color(theme::GOLD),
        );
        ui.add(egui::Label::new(RichText::new(text).color(theme::MUTED)).wrap());
    });
}

fn executable_name(path: Option<&Path>) -> String {
    path.and_then(Path::file_name)
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| EXECUTABLE.into())
}

fn session_label(count: u64) -> String {
    match count {
        0 => "Nenhuma sessão conectada".into(),
        1 => "1 sessão conectada".into(),
        count => format!("{count} sessões conectadas"),
    }
}

fn format_bytes(bytes: u64) -> String {
    match bytes {
        0..1024 => format!("{bytes} B"),
        1024..1_048_576 => format!("{:.1} KiB", bytes as f64 / 1024.0).replace('.', ","),
        _ => format!("{:.1} MiB", bytes as f64 / 1_048_576.0).replace('.', ","),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn play_link_is_available_only_after_the_listener_is_ready() {
        let view = View::from_snapshot(None);
        assert_eq!(view.title(), "Iniciando relay…");
        assert_eq!(view.primary_action(), Action::None);
        for (status, expected) in [
            (RelayStatus::Ready, Action::OpenPlayPage),
            (
                RelayStatus::Failed("Endereço já em uso".into()),
                Action::Retry,
            ),
            (RelayStatus::Stopped, Action::Retry),
        ] {
            let mut view = View::from_snapshot(None);
            view.status = &status;
            assert_eq!(view.primary_action(), expected);
        }
    }

    #[test]
    fn every_relay_state_fits_the_default_and_minimum_window_widths() {
        for size in [egui::vec2(420.0, 480.0), egui::vec2(600.0, 780.0)] {
            let ctx = egui::Context::default();
            theme::install(&ctx);
            for status in [
                RelayStatus::Launching,
                RelayStatus::Ready,
                RelayStatus::Failed(
                    "A porta 8787 já está em uso. Feche o outro relay e tente novamente.".into(),
                ),
                RelayStatus::Stopped,
            ] {
                let mut view = View::from_snapshot(None);
                view.status = &status;
                view.address = Some("127.0.0.1:8787".into());
                let output = ctx.run(
                    egui::RawInput {
                        screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
                        ..Default::default()
                    },
                    |ctx| assert_eq!(draw(ctx, &view), Action::None),
                );
                assert!(
                    !output
                        .platform_output
                        .commands
                        .iter()
                        .any(|command| matches!(command, egui::OutputCommand::OpenUrl(_)))
                );
                for shape in &output.shapes {
                    if let egui::Shape::Text(text) = &shape.shape {
                        let bounds = text.galley.rect.translate(text.pos.to_vec2());
                        assert!(bounds.left() >= 0.0, "{}", text.galley.text());
                        assert!(
                            bounds.right() <= size.x,
                            "text overflows the window: {}",
                            text.galley.text(),
                        );
                    }
                }
                let primitives = ctx.tessellate(output.shapes, output.pixels_per_point);
                assert!(!primitives.is_empty());
                for primitive in primitives {
                    if let egui::epaint::Primitive::Mesh(mesh) = primitive.primitive {
                        assert!(mesh.vertices.iter().all(|vertex| vertex.pos.is_finite()));
                    }
                }
            }
        }
    }

    #[test]
    fn session_and_transfer_labels_are_readable() {
        assert_eq!(session_label(0), "Nenhuma sessão conectada");
        assert_eq!(session_label(1), "1 sessão conectada");
        assert_eq!(session_label(3), "3 sessões conectadas");
        assert_eq!(format_bytes(987), "987 B");
        assert_eq!(format_bytes(1536), "1,5 KiB");
        assert_eq!(format_bytes(1_048_576), "1,0 MiB");
    }

    #[test]
    fn routing_instructions_identify_the_downloaded_or_renamed_executable() {
        assert_eq!(
            executable_name(Some(Path::new("Downloads/nwyd-relay-windows-x86_64.exe"))),
            "nwyd-relay-windows-x86_64.exe"
        );
        assert_eq!(
            executable_name(Some(Path::new("Downloads/my-relay.exe"))),
            "my-relay.exe"
        );
        assert_eq!(executable_name(None), EXECUTABLE);
    }

    #[test]
    fn primary_buttons_trigger_the_matching_action_on_click() {
        for (status, label, expected) in [
            (
                RelayStatus::Ready,
                "Abrir página do jogo",
                Action::OpenPlayPage,
            ),
            (RelayStatus::Stopped, "Iniciar relay", Action::Retry),
            (
                RelayStatus::Failed("Porta já em uso".into()),
                "Tentar novamente",
                Action::Retry,
            ),
        ] {
            let ctx = egui::Context::default();
            theme::install(&ctx);
            let mut view = View::from_snapshot(None);
            view.status = &status;
            let input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(560.0, 660.0),
                )),
                ..Default::default()
            };
            let output = ctx.run(input.clone(), |ctx| {
                draw(ctx, &view);
            });
            let position = output
                .shapes
                .iter()
                .find_map(|shape| match &shape.shape {
                    egui::Shape::Text(text) if text.galley.text() == label => {
                        Some(text.pos + text.galley.size() * 0.5)
                    }
                    _ => None,
                })
                .expect("primary action should be visible");

            for pressed in [true, false] {
                let mut action = Action::None;
                let _ = ctx.run(
                    egui::RawInput {
                        events: vec![
                            egui::Event::PointerMoved(position),
                            egui::Event::PointerButton {
                                pos: position,
                                button: egui::PointerButton::Primary,
                                pressed,
                                modifiers: egui::Modifiers::NONE,
                            },
                        ],
                        ..input.clone()
                    },
                    |ctx| action = draw(ctx, &view),
                );
                assert_eq!(action, if pressed { Action::None } else { expected });
            }
        }
    }
}
