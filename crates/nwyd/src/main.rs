#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

#[cfg(target_arch = "wasm32")]
fn main() {}

#[cfg(not(target_arch = "wasm32"))]
fn main() -> std::process::ExitCode {
    native::main()
}

#[cfg(not(target_arch = "wasm32"))]
mod native {

    use anyhow::{Context, Result};
    use assets::GameDir;
    use nwyd::error_context::Operation;
    use nwyd::window::{self, Client};
    use std::process::ExitCode;

    pub fn main() -> ExitCode {
        nwyd::logging::init();

        match run() {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => {
                tracing::error!(error = % format_args!("{error:#}"), "nwyd startup failed");

                let _ = nwyd::error_dialog::show(error, None);

                ExitCode::FAILURE
            }
        }
    }

    fn run() -> Result<()> {
        let event_loop = window::create_event_loop()?;

        let launcher_line = std::env::args_os()
            .skip(1)
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect::<Vec<_>>()
            .join(" ");

        let launcher = formats::launcher::parse(launcher_line.as_bytes())?
            .filter(formats::launcher::Credentials::enabled);

        let analytics = nwyd::analytics::configured(&nwyd::analytics::Config::from_env()?)?;

        let Some(mut client) = select_and_load_game(&analytics) else {
            return Ok(());
        };

        if let Some(credentials) = launcher {
            client = client.with_launcher(credentials);
        }

        client.run(event_loop)?;

        Ok(())
    }

    fn select_and_load_game(analytics: &nwyd::analytics::Analytics) -> Option<Client> {
        let language = nwyd::user_paths::settings()
            .ok()
            .and_then(|path| nwyd::game_settings::Preferences::load(&path).ok().flatten())
            .map_or(formats::config::Language::Portuguese, |preferences| {
                preferences.language
            });

        let portuguese = language == formats::config::Language::Portuguese;

        let mut directory = std::env::current_dir().ok();

        loop {
            let mut picker = rfd::FileDialog::new()
                .set_title(if portuguese {
                    "NWYD — Selecione a pasta com os arquivos do jogo"
                } else {
                    "NWYD — Select the folder containing the game files"
                })
                .set_can_create_directories(false);

            if let Some(directory) = &directory {
                picker = picker.set_directory(directory);
            }

            let root = picker.pick_folder()?;

            match GameDir::open(root.clone())
                .context(Operation::GameDirectory)
                .and_then(|game| nwyd::startup::load_client(game, analytics.clone()))
            {
                Ok(client) => return Some(client),
                Err(error) => {
                    tracing::warn!(path = % root.display(), error = % format_args!("{error:#}"), "could not load selected game directory");

                    let response = rfd::MessageDialog::new()
                    .set_title(if portuguese { "NWYD — Arquivos do jogo" } else { "NWYD — Game files" })
                    .set_description(if portuguese {
                        format!("Não foi possível carregar os arquivos dessa pasta.\n\n{error:#}\n\nSelecione a pasta que contém ItemList.bin e as pastas UI e Env.\n\nPressione OK para escolher outra pasta ou Cancelar para sair.")
                    } else {
                        format!("Could not load the files in this folder.\n\n{error:#}\n\nSelect the folder containing ItemList.bin and the UI and Env directories.\n\nPress OK to choose another folder or Cancel to exit.")
                    })
                    .set_level(rfd::MessageLevel::Warning)
                    .set_buttons(rfd::MessageButtons::OkCancel)
                    .show();

                    if response != rfd::MessageDialogResult::Ok {
                        return None;
                    }

                    directory = Some(root);
                }
            }
        }
    }
}
