use std::fmt;

#[macro_export]
macro_rules! internal_reason {
    ($code:literal, $description:literal) => {
        $description
    };
}

#[derive(Clone, Copy)]
pub enum Operation {
    GameDirectory,
    Scene,
    EventLoop,
    RunEventLoop,
    Window,
    Surface,
    Device,
    RecoverSurface,
    RecoverDevice,
}

impl fmt::Display for Operation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::GameDirectory => "indexing the game directory",
            Self::Scene => "building the scene",
            Self::EventLoop => {
                "creating the event loop (on Linux this needs DISPLAY or WAYLAND_DISPLAY set)"
            }
            Self::RunEventLoop => "running the event loop",
            Self::Window => "creating the window",
            Self::Surface => "creating the surface for the window",
            Self::Device => "opening a GPU device that can present to the window",
            Self::RecoverSurface => "recreating the surface after device loss",
            Self::RecoverDevice => "reopening the lost GPU device",
        };

        f.write_str(message)
    }
}
