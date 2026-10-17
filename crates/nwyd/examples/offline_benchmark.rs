use anyhow::{Context, Result, ensure};
use assets::GameDir;
use formats::config::Language;
use nwyd::clock::Calendar;
use nwyd::scene::Scene;
use nwyd::scene::cursor::Pointers;
use nwyd::scene::login_static::{self, LoginPresentation, SceneTime};
use nwyd::scene::server_select;
use nwyd::window::{Client, Settings, create_event_loop};
use render::device::RendererBackend;
use std::path::PathBuf;

fn main() -> Result<()> {
    let args = std::env::args_os().skip(1).collect::<Vec<_>>();

    ensure!(
        (2..=5).contains(&args.len()),
        "usage: offline_benchmark <game-folder> <report.json> [warmup=120] [frames=600] [world|static]"
    );

    let count = |index: usize, default: u64, name: &str| -> Result<u64> {
        match args.get(index) {
            Some(value) => value
                .to_str()
                .with_context(|| format!("{name} must be an unsigned integer"))?
                .parse()
                .with_context(|| format!("{name} must be an unsigned integer")),
            None => Ok(default),
        }
    };

    let warmup = count(2, 120, "warmup")?;

    let frames = count(3, 600, "frames")?;

    ensure!(frames > 0, "frames must be greater than zero");

    ensure!(warmup.checked_add(frames).is_some(), "frame count overflow");

    let kind = match args.get(4) {
        Some(value) => value.to_str().context("scene must be world or static")?,
        None => "world",
    };

    ensure!(
        matches!(kind, "world" | "static"),
        "scene must be world or static"
    );

    let mut game = GameDir::open(PathBuf::from(&args[0])).context("opening game files")?;

    game.set_language(Language::Portuguese);

    let backbuffer = (1280, 720);

    let time = SceneTime::pinned(10_000, Calendar::pinned_time(2, 12, 0, 0));

    let presentation = LoginPresentation::default();

    let scene = match kind {
        "static" => Scene::LoginStatic(login_static::build(
            &game,
            backbuffer,
            &presentation,
            &time,
        )?),
        _ => Scene::ServerSelect(Box::new(server_select::build(
            &game,
            backbuffer,
            &presentation,
            &time,
            true,
            Some(0),
        )?)),
    };

    let settings = Settings {
        title: format!("NWYD offline benchmark ({kind})"),
        backbuffer,
        fullscreen: false,
        present_mode: wgpu::PresentMode::Immediate,
        renderer: Some(RendererBackend::Vulkan),
        overlay: false,
        pointers: Pointers::none(),
    };

    Client::new(settings, scene)
        .with_render_quality(2, false)
        .with_error_dialogs(false)
        .with_frame_benchmark(warmup, frames, PathBuf::from(&args[1]))
        .run(create_event_loop()?)
}
