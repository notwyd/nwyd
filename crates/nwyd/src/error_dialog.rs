use std::fmt;
use winit::window::Window;

pub const TITLE: &str = "nwyd — Erro";

fn message(error: &anyhow::Error) -> String {
    format!("Não foi possível continuar o jogo.\n\n{error:#}\n\nFeche esta mensagem para sair.")
}

#[cfg(not(target_arch = "wasm32"))]

pub fn show(error: anyhow::Error, parent: Option<&Window>) -> anyhow::Error {
    present(error, |message| {
        let mut dialog = rfd::MessageDialog::new()
            .set_title(TITLE)
            .set_description(message)
            .set_level(rfd::MessageLevel::Error)
            .set_buttons(rfd::MessageButtons::OkCustom("Fechar".into()));

        if let Some(parent) = parent {
            parent.set_cursor_visible(true);

            dialog = dialog.set_parent(parent);
        }

        dialog.show();
    })
}

fn present(error: anyhow::Error, display: impl FnOnce(&str)) -> anyhow::Error {
    if error.chain().any(|cause| cause.is::<Presented>()) {
        return error;
    }

    display(&message(&error));

    Presented(error).into()
}

#[derive(Debug)]

struct Presented(anyhow::Error);

impl fmt::Display for Presented {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
    }
}

impl std::error::Error for Presented {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.0.source()
    }
}

#[cfg(target_arch = "wasm32")]

pub fn show(error: anyhow::Error, _parent: Option<&Window>) -> anyhow::Error {
    present(error, crate::web::report_error)
}
