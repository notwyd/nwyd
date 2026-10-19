use std::io::Cursor;
use symphonia::core::codecs::CodecParameters;
use symphonia::core::codecs::audio::{AudioDecoder, AudioDecoderOptions};
use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::formats::probe::Hint;
use symphonia::core::formats::{FormatOptions, FormatReader, SeekMode, SeekTo, TrackType};
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    Unlisted(usize),
    NoFile(String),
    Undecodable(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Unlisted(slot) => write!(f, "music slot {slot} names no path"),
            Error::NoFile(p) => write!(f, "{p} is not in the install"),
            Error::Undecodable(p) => write!(f, "{p} will not decode"),
        }
    }
}

impl std::error::Error for Error {}

pub struct Track {
    reader: Box<dyn FormatReader>,
    decoder: Box<dyn AudioDecoder>,
    track_id: u32,
    rate: u32,
    interleaved: Vec<f32>,
    buffer: Vec<[f32; 2]>,
    cursor: usize,
    decoded_since_rewind: bool,
    ended: bool,
}

impl std::fmt::Debug for Track {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Track")
            .field("rate", &self.rate)
            .field("buffered", &(self.buffer.len() - self.cursor))
            .field("ended", &self.ended)
            .finish()
    }
}

impl Track {
    pub fn open(bytes: Vec<u8>, name: &str) -> Result<Track, Error> {
        let source = MediaSourceStream::new(Box::new(Cursor::new(bytes)), Default::default());

        let mut hint = Hint::new();

        hint.with_extension("mp3");

        let undecodable = || Error::Undecodable(name.to_string());

        let reader = symphonia::default::get_probe()
            .probe(
                &hint,
                source,
                FormatOptions::default(),
                MetadataOptions::default(),
            )
            .map_err(|e| {
                tracing::warn!(track = name, error = % e, "music will not open");

                undecodable()
            })?;

        let track = reader
            .default_track(TrackType::Audio)
            .ok_or_else(undecodable)?;

        let track_id = track.id;

        let Some(CodecParameters::Audio(params)) = track.codec_params.as_ref() else {
            return Err(undecodable());
        };

        let rate = params.sample_rate.ok_or_else(undecodable)?;

        let decoder = symphonia::default::get_codecs()
            .make_audio_decoder(params, &AudioDecoderOptions::default())
            .map_err(|e| {
                tracing::warn!(track = name, error = % e, "no decoder for music");

                undecodable()
            })?;

        Ok(Track {
            reader,
            decoder,
            track_id,
            rate,
            interleaved: Vec::new(),
            buffer: Vec::new(),
            cursor: 0,
            decoded_since_rewind: false,
            ended: false,
        })
    }

    pub fn rate(&self) -> u32 {
        self.rate
    }

    pub fn ended(&self) -> bool {
        self.ended
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn buffer_frames(
        &mut self,
        count: usize,
        mut should_stop: impl FnMut() -> bool,
    ) -> bool {
        if should_stop() {
            return false;
        }

        if self.buffer.len() - self.cursor >= count || self.ended {
            return true;
        }

        if self.cursor != 0 {
            self.buffer.copy_within(self.cursor.., 0);

            self.buffer.truncate(self.buffer.len() - self.cursor);

            self.cursor = 0;
        }

        while self.buffer.len() < count && !self.ended {
            if !self.decode_packet(&mut should_stop) {
                return false;
            }
        }

        true
    }

    pub fn next_frame(&mut self) -> Option<[f32; 2]> {
        loop {
            if let Some(&frame) = self.buffer.get(self.cursor) {
                self.cursor += 1;

                return Some(frame);
            }

            if self.ended {
                return None;
            }

            self.fill();
        }
    }

    fn fill(&mut self) {
        self.buffer.clear();

        self.cursor = 0;

        self.decode_packet(&mut || false);
    }

    fn decode_packet(&mut self, should_stop: &mut impl FnMut() -> bool) -> bool {
        let packet = loop {
            if should_stop() {
                return false;
            }

            match self.reader.next_packet() {
                Ok(Some(p)) if p.track_id == self.track_id => break p,
                Ok(Some(_)) => continue,
                Ok(None) => {
                    self.rewind(None);

                    return true;
                }
                Err(e) => {
                    self.rewind(Some(&e));

                    return true;
                }
            }
        };

        match self.decoder.decode(&packet) {
            Ok(decoded) => {
                let channels = decoded.spec().channels().count().max(1);

                decoded.copy_to_vec_interleaved(&mut self.interleaved);

                self.buffer
                    .extend(self.interleaved.chunks(channels).map(|c| match c {
                        [m] => [*m, *m],
                        [l, r, ..] => [*l, *r],
                        _ => [0.0, 0.0],
                    }));

                self.decoded_since_rewind |= !self.interleaved.is_empty();
            }
            Err(SymphoniaError::DecodeError(e)) => {
                tracing::debug!(error = % e, "skipping a corrupt music packet");
            }
            Err(e) => self.rewind(Some(&e)),
        }

        true
    }

    fn rewind(&mut self, cause: Option<&SymphoniaError>) {
        let end_of_stream = match cause {
            None => true,
            Some(SymphoniaError::IoError(e)) => e.kind() == std::io::ErrorKind::UnexpectedEof,
            Some(SymphoniaError::ResetRequired) => true,
            Some(_) => false,
        };

        if !end_of_stream {
            if let Some(e) = cause {
                tracing::debug!(error = % e, "music stream stopped");
            }

            self.ended = true;

            return;
        }

        if !self.decoded_since_rewind {
            tracing::debug!("music stream contains no decodable audio; stopping");

            self.ended = true;

            return;
        }

        let time = symphonia::core::units::Time::try_new(0, 0).expect("zero is a valid Time");

        let to = SeekTo::Time {
            time,
            track_id: Some(self.track_id),
        };

        match self.reader.seek(SeekMode::Accurate, to) {
            Ok(_) => {
                self.decoder.reset();

                self.decoded_since_rewind = false;
            }
            Err(e) => {
                tracing::debug!(error = % e, "music will not loop; stopping");

                self.ended = true;
            }
        }
    }
}

#[cfg(test)]
pub(crate) mod behavior_tests;
