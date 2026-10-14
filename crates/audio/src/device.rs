use crate::mixer::{CHANNELS, Mixer};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    NoDevice,
    NoConfig(String),
    UnsupportedFormat(String),
    Stream(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::NoDevice => write!(f, "no default output device"),
            Error::NoConfig(e) => write!(f, "the device would not report a config: {e}"),
            Error::UnsupportedFormat(s) => write!(f, "unsupported sample format {s}"),
            Error::Stream(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for Error {}

pub struct Output {
    #[cfg(target_arch = "wasm32")]
    _browser_unlock: BrowserUnlock,
    _stream: cpal::Stream,
    rate: u32,
    channels: u16,
}

impl std::fmt::Debug for Output {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Output")
            .field("rate", &self.rate)
            .field("channels", &self.channels)
            .finish()
    }
}

impl Output {
    pub fn rate(&self) -> u32 {
        self.rate
    }

    pub fn channels(&self) -> u16 {
        self.channels
    }
}

pub fn default_config() -> Result<(u32, u16, cpal::SampleFormat), Error> {
    let host = cpal::default_host();

    let device = host.default_output_device().ok_or(Error::NoDevice)?;

    let config = device
        .default_output_config()
        .map_err(|e| Error::NoConfig(e.to_string()))?;

    Ok((
        config.sample_rate(),
        config.channels(),
        config.sample_format(),
    ))
}

pub fn open(mixer: Arc<Mutex<Mixer>>) -> Result<Output, Error> {
    let host = cpal::default_host();

    let device = host.default_output_device().ok_or(Error::NoDevice)?;

    let supported = device
        .default_output_config()
        .map_err(|e| Error::NoConfig(e.to_string()))?;

    let rate = supported.sample_rate();

    let channels = supported.channels();

    let format = supported.sample_format();

    let config: cpal::StreamConfig = supported.into();

    let err_fn = |e| tracing::warn!(error = % e, "audio output stream error");

    let mut scratch: Vec<f32> = Vec::new();

    let stream = match format {
        cpal::SampleFormat::F32 => device.build_output_stream(
            config,
            move |out: &mut [f32], _| {
                let frames = out.len() / channels as usize;

                scratch.resize(frames * CHANNELS, 0.0);

                fill(&mixer, &mut scratch);

                scatter(&scratch, out, channels as usize, |s| s);
            },
            err_fn,
            None,
        ),
        cpal::SampleFormat::I16 => device.build_output_stream(
            config,
            move |out: &mut [i16], _| {
                let frames = out.len() / channels as usize;

                scratch.resize(frames * CHANNELS, 0.0);

                fill(&mixer, &mut scratch);

                scatter(&scratch, out, channels as usize, |s| {
                    (s * i16::MAX as f32) as i16
                });
            },
            err_fn,
            None,
        ),
        other => return Err(Error::UnsupportedFormat(format!("{other:?}"))),
    }
    .map_err(|e| Error::Stream(e.to_string()))?;

    stream.play().map_err(|e| Error::Stream(e.to_string()))?;

    Ok(Output {
        #[cfg(target_arch = "wasm32")]
        _browser_unlock: BrowserUnlock::new(&stream)?,
        _stream: stream,
        rate,
        channels,
    })
}

fn fill(mixer: &Arc<Mutex<Mixer>>, scratch: &mut [f32]) {
    match mixer.lock() {
        Ok(mut m) => m.fill(scratch),
        Err(_) => scratch.fill(0.0),
    }
}

fn scatter<T: Copy + Default>(
    stereo: &[f32],
    out: &mut [T],
    channels: usize,
    to: impl Fn(f32) -> T,
) {
    out.fill(T::default());

    if channels == 0 {
        return;
    }

    for (frame, chunk) in out.chunks_mut(channels).enumerate() {
        let l = stereo.get(frame * CHANNELS).copied().unwrap_or(0.0);

        let r = stereo.get(frame * CHANNELS + 1).copied().unwrap_or(l);

        match channels {
            1 => chunk[0] = to((l + r) * 0.5),
            _ => {
                chunk[0] = to(l);

                chunk[1] = to(r);
            }
        }
    }
}

/// Asset loading outlives the folder picker's user activation. Resume the audio
/// context from a later trusted gesture, including after a browser interruption.
#[cfg(target_arch = "wasm32")]
struct BrowserUnlock {
    window: web_sys::Window,
    resume: wasm_bindgen::closure::Closure<dyn FnMut(web_sys::Event)>,
}

#[cfg(target_arch = "wasm32")]
impl BrowserUnlock {
    const EVENTS: [&'static str; 3] = ["pointerdown", "pointerup", "keydown"];

    fn new(stream: &cpal::Stream) -> Result<Self, Error> {
        use wasm_bindgen::JsCast;

        let cpal::platform::StreamInner::WebAudio(stream) = stream.as_inner();

        let context = stream.audio_context().clone();

        let window = web_sys::window()
            .ok_or_else(|| Error::Stream("browser window is unavailable".into()))?;

        let resume = wasm_bindgen::closure::Closure::new(move |event: web_sys::Event| {
            if !event.is_trusted()
                || matches!(
                    context.state(),
                    web_sys::AudioContextState::Running | web_sys::AudioContextState::Closed
                )
            {
                return;
            }

            // Call resume during the event itself, while user activation is live.
            let result = context.resume();

            wasm_bindgen_futures::spawn_local(async move {
                if let Ok(promise) = result {
                    if let Err(error) = wasm_bindgen_futures::JsFuture::from(promise).await {
                        tracing::debug!(?error, "browser audio is waiting for another gesture");
                    }
                }
            });
        });

        let unlock = Self { window, resume };

        for event in Self::EVENTS {
            unlock
                .window
                .add_event_listener_with_callback_and_bool(
                    event,
                    unlock.resume.as_ref().unchecked_ref(),
                    true,
                )
                .map_err(|error| Error::Stream(format!("audio gesture listener: {error:?}")))?;
        }

        Ok(unlock)
    }
}

#[cfg(target_arch = "wasm32")]
impl Drop for BrowserUnlock {
    fn drop(&mut self) {
        use wasm_bindgen::JsCast;

        for event in Self::EVENTS {
            let _ = self.window.remove_event_listener_with_callback_and_bool(
                event,
                self.resume.as_ref().unchecked_ref(),
                true,
            );
        }
    }
}
