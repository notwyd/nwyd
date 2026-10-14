use super::Client;
use crate::{
    nell_browser,
    scene::{Scene, field, notice},
};

impl Client {
    pub(super) fn flush_nell_browser(&mut self) {
        let visible = self.nell_browser_visible();

        if !visible || self.nell_browser.as_ref().is_some_and(|host| host.closed()) {
            let was_open = self.nell_browser.take().is_some();

            if let Scene::Field(scene) = &mut self.runtime.scene {
                scene.nell_browser.close();
            }

            if was_open {
                if let Some(live) = &self.live {
                    live.window.focus_window();
                }

                self.sync_ime();

                self.redraw();
            }

            return;
        }

        if let Some(host) = &mut self.nell_browser {
            host.poll();

            #[cfg(target_os = "windows")]
            let _ = host.reposition();

            #[cfg(any(target_os = "linux", target_os = "macos"))]
            if let Some(live) = &self.live {
                let _ = host.reposition(&live.window);
            }

            return;
        }

        let Scene::Field(scene) = &mut self.runtime.scene else {
            return;
        };

        let Some(empty) = scene.nell_browser.pending.take() else {
            return;
        };

        field::cancel_pointer_hold(scene);

        self.runtime.input.mouse = Default::default();

        self.runtime.input.mods = Default::default();

        if let Some(hud) = &mut scene.hud {
            hud.manager.keys = Default::default();
        }

        let context = (|| {
            let base = scene.panels.as_ref()?.strings.translate("ss_269");

            let url = nell_browser::service_url(
                &base,
                &self.runtime.net.account,
                self.picked.as_ref()?.server,
                empty,
                self.launcher_credentials.as_ref().map_or(0, |c| c.kind),
            )?;

            let window = &self.live.as_ref()?.window;

            let exe = self
                .screens
                .as_ref()?
                .game()
                .resolve(b"WYD.exe")
                .unwrap_or_default();

            Some((window, url, exe))
        })();

        let result = context.map(|(parent, url, exe)| nell_browser::open(parent, &url, &exe));

        match result {
            Some(Ok(host)) => self.nell_browser = Some(host),
            failure => {
                #[cfg(target_os = "windows")]
                {
                    let code = failure.and_then(Result::err).and_then(|error| {
                        error
                            .downcast_ref::<windows::core::Error>()
                            .map(|error| error.code().0)
                    });

                    tracing::warn!(?code, "NWYD: could not open the NPC browser");
                }

                #[cfg(not(target_os = "windows"))]
                let _ = failure;

                #[cfg(not(target_os = "windows"))]
                tracing::warn!("NWYD: could not open the NPC browser");

                scene.nell_browser.close();

                if let Some(panels) = &mut scene.panels {
                    notice::post(
                        panels,
                        "Não foi possível abrir o navegador de Prisang Nell.",
                        5000,
                        true,
                        scene.time.now_ms,
                    );
                }
            }
        }
    }
}
