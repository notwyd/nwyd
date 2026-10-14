use anyhow::Result;
use std::{cell::Cell, rc::Rc};
use winit::window::Window;
use wry::{WebView, WebViewBuilder};

const ESCAPE_SCRIPT: &str = r#"
document.addEventListener('keydown', function (event) {
  if (event.key === 'Escape' || event.keyCode === 27) {
    event.preventDefault(); window.ipc.postMessage('nell-close');
  }
}, true);
"#;

pub struct Browser {
    view: WebView,
    #[cfg(target_os = "macos")]
    footer: WebView,
    #[cfg(target_os = "linux")]
    _window: GtkHost,
    closed: Rc<Cell<bool>>,
}

#[cfg(target_os = "linux")]

struct GtkHost(gtk::Window);

#[cfg(target_os = "linux")]

impl Drop for GtkHost {
    fn drop(&mut self) {
        use gtk::prelude::*;

        let display = self.0.display();

        unsafe {
            self.0.destroy();
        }

        display.flush();
    }
}

impl Browser {
    pub fn open(parent: &Window, url: &str) -> Result<Self> {
        let closed = Rc::new(Cell::new(false));

        let close = closed.clone();

        let builder = WebViewBuilder::new()
            .with_user_agent(super::USER_AGENT)
            .with_devtools(false)
            .with_initialization_script(ESCAPE_SCRIPT)
            .with_ipc_handler(move |request| {
                if request.body() == "nell-close" {
                    close.set(true);
                }
            });

        #[cfg(target_os = "linux")]
        {
            use gtk::prelude::*;
            use webkit2gtk::SettingsExt;
            use wry::{WebViewBuilderExtUnix, WebViewExtUnix};

            let _ = parent;

            gtk::init().map_err(|_| anyhow::anyhow!("Could not initialize the NPC browser"))?;

            let window = gtk::Window::new(gtk::WindowType::Toplevel);

            let owner = GtkHost(window.clone());

            window.set_title("Prisang Nell");

            window.set_default_size(800, 581);

            window.set_resizable(false);

            let content = gtk::Box::new(gtk::Orientation::Vertical, 0);

            let web_area = gtk::Box::new(gtk::Orientation::Vertical, 0);

            web_area.set_size_request(800, 560);

            let close_button = gtk::Button::with_label("Fechar");

            close_button.set_size_request(86, 21);

            close_button.set_halign(gtk::Align::End);

            let close = closed.clone();

            close_button.connect_clicked(move |_| close.set(true));

            let close = closed.clone();

            window.connect_delete_event(move |_, _| {
                close.set(true);

                gtk::glib::Propagation::Stop
            });

            let close = closed.clone();

            window.connect_key_press_event(move |_, event| {
                if event.keyval() == gtk::gdk::keys::constants::Escape {
                    close.set(true);

                    gtk::glib::Propagation::Stop
                } else {
                    gtk::glib::Propagation::Proceed
                }
            });

            content.pack_start(&web_area, true, true, 0);

            content.pack_start(&close_button, false, false, 0);

            window.add(&content);

            let view = builder.with_visible(false).build_gtk(&web_area)?;

            let settings = webkit2gtk::WebViewExt::settings(&view.webview())
                .ok_or_else(|| anyhow::anyhow!("Could not configure the NPC browser"))?;

            settings
                .set_hardware_acceleration_policy(webkit2gtk::HardwareAccelerationPolicy::Never);

            view.load_url(url)?;

            view.set_visible(true)?;

            window.show_all();

            window.present();

            Ok(Self {
                view,
                _window: owner,
                closed,
            })
        }

        #[cfg(target_os = "macos")]
        {
            let (body, bar) = bounds(parent);

            let view = builder
                .with_url(url)
                .with_bounds(body)
                .build_as_child(parent)?;

            let close = closed.clone();

            let footer = WebViewBuilder::new()
                .with_user_agent(super::USER_AGENT)
                .with_devtools(false)
                .with_initialization_script(ESCAPE_SCRIPT)
                .with_ipc_handler(move |request| {
                    if request.body() == "nell-close" {
                        close.set(true);
                    }
                })
                .with_html(
                    "<!doctype html><meta charset=utf-8><style>body{margin:0;background:#ddd}button{float:right;width:86px;height:21px;padding:0}</style><button onclick=\"window.ipc.postMessage('nell-close')\">Fechar</button>",
                )
                .with_bounds(bar)
                .build_as_child(parent)?;

            view.focus()?;

            Ok(Self {
                view,
                footer,
                closed,
            })
        }
    }

    pub fn closed(&self) -> bool {
        self.closed.get()
    }

    pub fn close(&self) {
        self.closed.set(true);
    }

    pub fn poll(&mut self) {
        #[cfg(target_os = "linux")]
        for _ in 0..64 {
            if !gtk::events_pending() {
                break;
            }

            gtk::main_iteration_do(false);
        }
    }

    pub fn reposition(&self, parent: &Window) -> Result<()> {
        #[cfg(target_os = "linux")]
        let _ = parent;

        #[cfg(target_os = "macos")]
        {
            let (body, bar) = bounds(parent);

            self.view.set_bounds(body)?;

            self.footer.set_bounds(bar)?;
        }

        Ok(())
    }
}

#[cfg(target_os = "macos")]

fn bounds(parent: &Window) -> (wry::Rect, wry::Rect) {
    use wry::dpi::{LogicalPosition, LogicalSize};

    let size = parent.inner_size().to_logical::<f64>(parent.scale_factor());

    let x = ((size.width - 800.0) / 2.0).max(0.0);

    let y = ((size.height - 581.0) / 2.0).max(0.0);

    (
        wry::Rect {
            position: LogicalPosition::new(x, y).into(),
            size: LogicalSize::new(800, 560).into(),
        },
        wry::Rect {
            position: LogicalPosition::new(x, y + 560.0).into(),
            size: LogicalSize::new(800, 21).into(),
        },
    )
}

impl Drop for Browser {
    fn drop(&mut self) {
        let _ = self.view.load_url("about:blank");
    }
}
