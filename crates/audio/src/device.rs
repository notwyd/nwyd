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
