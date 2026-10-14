use super::*;
use bank::FRAME;
use formats::audio::sound_list;
use std::cell::Cell;

fn riff(chunks: Vec<([u8; 4], Vec<u8>)>) -> Vec<u8> {
    let mut bytes = b"RIFF\0\0\0\0WAVE".to_vec();

    for (id, body) in chunks {
        bytes.extend_from_slice(&id);

        bytes.extend_from_slice(&(body.len() as u32).to_le_bytes());

        bytes.extend_from_slice(&body);

        if body.len() % 2 != 0 {
            bytes.push(0);
        }
    }

    let size = (bytes.len() - 8) as u32;

    bytes[4..8].copy_from_slice(&size.to_le_bytes());

    bytes
}

fn pcm_format(tag: u16, channels: u16, rate: u32, bits: u16) -> Vec<u8> {
    let alignment = channels * bits / 8;

    let mut bytes = Vec::new();

    bytes.extend_from_slice(&tag.to_le_bytes());

    bytes.extend_from_slice(&channels.to_le_bytes());

    bytes.extend_from_slice(&rate.to_le_bytes());

    bytes.extend_from_slice(&(rate * u32::from(alignment)).to_le_bytes());

    bytes.extend_from_slice(&alignment.to_le_bytes());

    bytes.extend_from_slice(&bits.to_le_bytes());

    bytes
}

fn pcm(channels: u16, rate: u32, bits: u16, data: &[u8]) -> Vec<u8> {
    riff(vec![
        (*b"fmt ", pcm_format(1, channels, rate, bits)),
        (*b"data", data.to_vec()),
    ])
}

struct TestLoader {
    bytes: Option<Vec<u8>>,
    reads: Cell<usize>,
}

impl TestLoader {
    fn new(bytes: Option<Vec<u8>>) -> Self {
        Self {
            bytes,
            reads: Cell::new(0),
        }
    }
}

impl Loader for TestLoader {
    fn read(&self, path: &str) -> Option<Vec<u8>> {
        assert_eq!(path, "sound/test.wav");

        self.reads.set(self.reads.get() + 1);

        self.bytes.clone()
    }
}

fn bank() -> Bank {
    Bank::new(
        sound_list::parse(b"1 sound\\test.wav 2\n").unwrap(),
        volume::MAX,
    )
}

fn mono_loader(rate: u32, samples: &[i16]) -> TestLoader {
    let bytes: Vec<u8> = samples
        .iter()
        .flat_map(|sample| sample.to_le_bytes())
        .collect();

    TestLoader::new(Some(pcm(1, rate, 16, &bytes)))
}

fn assert_samples(actual: &[f32], expected: &[f32]) {
    assert_eq!(actual.len(), expected.len());

    for (index, (&actual, &expected)) in actual.iter().zip(expected).enumerate() {
        assert!(
            (actual - expected).abs() < 0.00001,
            "sample {index}: {actual} != {expected}"
        );
    }
}

#[test]
fn wave_decodes_unsigned_eight_bit_mono_without_padding_samples() {
    let wave = wave::parse(&pcm(1, 8000, 8, &[0, 128, 255])).unwrap();

    assert_eq!(wave.channels, 1);

    assert_eq!(wave.rate, 8000);

    assert_eq!(wave.samples, [-32768, 0, 32512]);

    assert_eq!(wave.frame(2), [32512; 2]);

    assert_eq!(wave.frame(3), [0; 2]);
}

#[test]
fn wave_decodes_signed_little_endian_stereo() {
    let samples = [i16::MIN, i16::MAX, -1234, 5678];

    let data: Vec<_> = samples.into_iter().flat_map(i16::to_le_bytes).collect();

    let wave = wave::parse(&pcm(2, 2, 16, &data)).unwrap();

    assert_eq!(wave.samples, samples);

    assert_eq!(wave.frames(), 2);

    assert_eq!(wave.duration_ms(), 1000);

    assert_eq!(wave.frame(0), [i16::MIN, i16::MAX]);

    assert_eq!(wave.frame(1), [-1234, 5678]);

    assert_eq!(wave.frame(2), [0; 2]);
}

#[test]
fn wave_skips_unknown_odd_sized_chunks() {
    let bytes = riff(vec![
        (*b"JUNK", vec![1, 2, 3]),
        (*b"fmt ", pcm_format(1, 1, 8000, 16)),
        (*b"data", vec![0x7f, 0]),
    ]);

    assert_eq!(wave::parse(&bytes).unwrap().samples, [127]);
}

#[test]
fn wave_accepts_data_before_format() {
    let bytes = riff(vec![
        (*b"data", vec![0xff, 0x7f]),
        (*b"fmt ", pcm_format(1, 1, 8000, 16)),
    ]);

    assert_eq!(wave::parse(&bytes).unwrap().samples, [32767]);
}

#[test]
fn wave_rejects_invalid_and_truncated_container_headers() {
    let valid = pcm(1, 8000, 16, &[0, 0]);

    for length in 0..12 {
        assert_eq!(wave::parse(&valid[..length]), Err(wave::Error::NotRiff));
    }

    let mut invalid = valid;

    invalid[8..12].copy_from_slice(b"NONE");

    assert_eq!(wave::parse(&invalid), Err(wave::Error::NotRiff));
}

#[test]
fn wave_rejects_chunk_payloads_past_the_end() {
    let mut bytes = pcm(1, 8000, 16, &[0, 0]);

    bytes.pop();

    assert_eq!(wave::parse(&bytes), Err(wave::Error::Truncated));

    let mut bytes = riff(vec![(*b"data", vec![])]);

    bytes[16..20].copy_from_slice(&u32::MAX.to_le_bytes());

    assert_eq!(wave::parse(&bytes), Err(wave::Error::Truncated));
}

#[test]
fn wave_requires_both_format_and_data_chunks() {
    assert_eq!(
        wave::parse(&riff(vec![(*b"data", vec![0, 0])])),
        Err(wave::Error::NoFormat)
    );

    assert_eq!(
        wave::parse(&riff(vec![(*b"fmt ", pcm_format(1, 1, 8000, 16))])),
        Err(wave::Error::NoData)
    );

    assert_eq!(
        wave::parse(&riff(vec![(*b"fmt ", vec![0; 15]), (*b"data", vec![0, 0])])),
        Err(wave::Error::NoFormat)
    );
}

#[test]
fn wave_rejects_compression_zero_rate_and_unsupported_layouts() {
    let compressed = riff(vec![
        (*b"fmt ", pcm_format(3, 1, 8000, 16)),
        (*b"data", vec![0, 0]),
    ]);

    assert_eq!(wave::parse(&compressed), Err(wave::Error::NotPcm(3)));

    assert_eq!(
        wave::parse(&pcm(1, 0, 16, &[0, 0])),
        Err(wave::Error::ZeroRate)
    );

    for (channels, bits) in [(0, 16), (3, 16), (1, 24)] {
        assert_eq!(
            wave::parse(&pcm(channels, 8000, bits, &[0; 6])),
            Err(wave::Error::Unsupported { channels, bits })
        );
    }
}

#[test]
fn volume_mapping_clamps_percentages_and_uses_decibels() {
    assert_eq!(volume::sfx(-50), -2500);

    assert_eq!(volume::sfx(0), -2500);

    assert_eq!(volume::sfx(50), -1250);

    assert_eq!(volume::sfx(100), 0);

    assert_eq!(volume::sfx(150), 0);

    assert_eq!(volume::music(-500), -10000);

    assert_eq!(volume::music(0), -3000);

    assert_eq!(volume::music(50), -1500);

    assert_eq!(volume::music(150), 0);

    assert_eq!(volume::amplitude(volume::MIN), 0.0);

    assert_eq!(volume::amplitude(volume::MIN - 1), 0.0);

    assert_eq!(volume::amplitude(0), 1.0);

    assert!((volume::amplitude(-2000) - 0.1).abs() < 0.00001);
}

#[test]
fn bank_refuses_invalid_and_muted_requests_without_loading() {
    let loader = TestLoader::new(None);

    let mut bank = bank();

    for index in [-1, 0, 512, i64::MAX] {
        assert_eq!(bank.sound(index, &loader).unwrap_err(), Refusal::OutOfRange);
    }

    assert_eq!(bank.sound(2, &loader).unwrap_err(), Refusal::Unlisted);

    bank.set_volume(volume::MIN);

    assert_eq!(bank.sound(1, &loader).unwrap_err(), Refusal::Muted);

    assert_eq!(loader.reads.get(), 0);
}

#[test]
fn bank_caches_loaded_sounds_until_release() {
    let loader = mono_loader(8000, &[1000, 2000]);

    let mut bank = bank();

    assert_eq!(bank.sound(1, &loader).unwrap().voices(), 2);

    bank.sound(1, &loader).unwrap();

    assert_eq!(loader.reads.get(), 1);

    assert_eq!(bank.resident().collect::<Vec<_>>(), [1]);

    bank.release(1);

    assert!(bank.loaded(1).is_none());

    bank.sound(1, &loader).unwrap();

    assert_eq!(loader.reads.get(), 2);
}

#[test]
fn bank_caches_failed_loads_and_release_allows_retry() {
    let mut loader = TestLoader::new(None);

    let mut bank = bank();

    let expected = Refusal::NoFile("sound/test.wav".to_owned());

    assert_eq!(bank.sound(1, &loader).unwrap_err(), expected);

    loader.bytes = Some(pcm(1, 8000, 8, &[128]));

    assert_eq!(bank.sound(1, &loader).unwrap_err(), expected);

    assert_eq!(loader.reads.get(), 1);

    bank.release(1);

    assert_eq!(bank.sound(1, &loader).unwrap().wave().samples, [0]);

    assert_eq!(loader.reads.get(), 2);
}

#[test]
fn bank_rejects_undecodable_sound_data() {
    let loader = TestLoader::new(Some(b"synthetic invalid wave".to_vec()));

    let mut bank = bank();

    assert_eq!(
        bank.sound(1, &loader).unwrap_err(),
        Refusal::Undecodable("sound/test.wav".to_owned())
    );

    assert!(bank.resident().next().is_none());
}

#[test]
fn sound_allocates_free_voices_and_stops_at_the_end() {
    let loader = mono_loader(8000, &[1000, 2000]);

    let mut bank = bank();

    let sound = bank.sound(1, &loader).unwrap();

    assert_eq!(sound.play(false), 0);

    assert_eq!(sound.play(false), 1);

    sound.advance_fixed(0, 2 * FRAME);

    assert_eq!(sound.active_fixed().collect::<Vec<_>>(), [(1, 0, false)]);

    assert_eq!(sound.play(false), 0);

    sound.stop();

    assert!(!sound.is_playing());
}

#[test]
fn looping_sound_wraps_across_multiple_durations() {
    let loader = mono_loader(8000, &[1000, 2000]);

    let mut bank = bank();

    let sound = bank.sound(1, &loader).unwrap();

    let voice = sound.play(true);

    sound.advance_fixed(voice, 5 * FRAME + FRAME / 2);

    assert_eq!(
        sound.active_fixed().collect::<Vec<_>>(),
        [(voice, FRAME + FRAME / 2, true)]
    );

    assert!(sound.is_playing());
}

#[test]
fn empty_wave_stops_even_a_looping_voice() {
    let loader = mono_loader(8000, &[]);

    let mut bank = bank();

    let sound = bank.sound(1, &loader).unwrap();

    let voice = sound.play(true);

    sound.advance_fixed(voice, FRAME);

    assert!(!sound.is_playing());
}

#[test]
fn mixer_resamples_mono_to_stereo_with_linear_interpolation() {
    let loader = mono_loader(2, &[0, 16000]);

    let mut mixer = Mixer::new(bank(), volume::MAX, 4);

    mixer.play(1, &loader).unwrap();

    let mut output = [0.0; 8];

    mixer.fill(&mut output);

    let expected = [0.0, 0.0, 8000.0, 8000.0, 16000.0, 16000.0, 8000.0, 8000.0]
        .map(|sample| sample / i16::MAX as f32);

    assert_samples(&output, &expected);

    assert!(!mixer.is_playing(1));
}

#[test]
fn inactive_mixer_emits_silence_without_consuming_voices() {
    let loader = mono_loader(8000, &[1000, 2000]);

    let mut mixer = Mixer::new(bank(), volume::MAX, 8000);

    mixer.play(1, &loader).unwrap();

    mixer.set_active(false);

    let mut output = [1.0; 4];

    mixer.fill(&mut output);

    assert_eq!(output, [0.0; 4]);

    assert!(mixer.is_playing(1));

    mixer.set_active(true);

    mixer.fill(&mut output);

    assert_samples(
        &output,
        &[1000.0, 1000.0, 2000.0, 2000.0].map(|sample| sample / i16::MAX as f32),
    );

    assert!(!mixer.is_playing(1));
}

#[test]
fn mixer_clips_overlapping_voices_and_clears_unpaired_output() {
    let samples = [30000_i16, -30000];

    let bytes: Vec<_> = samples.into_iter().flat_map(i16::to_le_bytes).collect();

    let loader = TestLoader::new(Some(pcm(2, 8000, 16, &bytes)));

    let mut mixer = Mixer::new(bank(), volume::MAX, 8000);

    mixer.play(1, &loader).unwrap();

    mixer.play(1, &loader).unwrap();

    let mut output = [99.0; 3];

    mixer.fill(&mut output);

    assert_eq!(output, [1.0, -1.0, 0.0]);

    mixer.fill(&mut output);

    assert_eq!(output, [0.0; 3]);
}

#[test]
fn repeated_loop_requests_preserve_the_current_playback_position() {
    let loader = mono_loader(8000, &[1000, 2000]);

    let mut mixer = Mixer::new(bank(), volume::MAX, 8000);

    mixer.play_looping(1, &loader).unwrap();

    mixer.fill(&mut [0.0; 2]);

    mixer.play_looping(1, &loader).unwrap();

    assert_eq!(
        mixer.bank.loaded(1).unwrap().active().collect::<Vec<_>>(),
        [(1, true)]
    );

    mixer.stop(1);

    assert!(!mixer.is_playing(1));
}

#[test]
fn silent_engine_plays_without_an_audio_device() {
    let sounds = sound_list::parse(b"1 sound/test.wav 2").unwrap();

    let engine = Engine::silent(sounds, MusicTable::default(), 100, 100);

    let loader = mono_loader(DEFAULT_RATE, &[1000, 2000]);

    assert!(engine.output().is_none());

    engine.play_looping(1, &loader);

    assert!(engine.is_playing(1));

    engine.set_active(false);

    let mut output = [1.0; 4];

    engine.mixer().fill(&mut output);

    assert_eq!(output, [0.0; 4]);

    engine.release(1);

    assert!(!engine.is_playing(1));
}

#[test]
fn invalid_music_data_reports_the_requested_asset_name() {
    assert_eq!(
        Track::open(b"synthetic invalid mp3".to_vec(), "test.mp3").unwrap_err(),
        music::Error::Undecodable("test.mp3".to_owned())
    );
}
