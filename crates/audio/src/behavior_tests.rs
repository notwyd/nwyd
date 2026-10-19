use crate::bank::FRAME;
use crate::{Bank, Loader, Mixer, Refusal, volume};
use formats::audio::sound_list;
use std::cell::Cell;
use std::collections::BTreeMap;

fn pcm(channels: u16, rate: u32, samples: &[i16]) -> Vec<u8> {
    let data_len = (samples.len() * 2) as u32;
    let mut bytes = b"RIFF".to_vec();
    bytes.extend_from_slice(&(36 + data_len).to_le_bytes());
    bytes.extend_from_slice(b"WAVEfmt ");
    bytes.extend_from_slice(&16_u32.to_le_bytes());
    bytes.extend_from_slice(&1_u16.to_le_bytes());
    bytes.extend_from_slice(&channels.to_le_bytes());
    bytes.extend_from_slice(&rate.to_le_bytes());
    bytes.extend_from_slice(&(rate * u32::from(channels) * 2).to_le_bytes());
    bytes.extend_from_slice(&(channels * 2).to_le_bytes());
    bytes.extend_from_slice(&16_u16.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&data_len.to_le_bytes());
    bytes.extend(samples.iter().flat_map(|sample| sample.to_le_bytes()));
    bytes
}

struct Assets {
    files: BTreeMap<&'static str, Vec<u8>>,
    reads: Cell<usize>,
}

impl Assets {
    fn new<const N: usize>(files: [(&'static str, Vec<u8>); N]) -> Self {
        Self {
            files: files.into_iter().collect(),
            reads: Cell::new(0),
        }
    }
}

impl Loader for Assets {
    fn read(&self, path: &str) -> Option<Vec<u8>> {
        self.reads.set(self.reads.get() + 1);
        self.files.get(path).cloned()
    }
}

fn mixer(list: &[u8], rate: u32) -> Mixer {
    Mixer::new(
        Bank::new(sound_list::parse(list).unwrap(), volume::MAX),
        volume::MAX,
        rate,
    )
}

fn render(mixer: &mut Mixer, frames: usize) -> Vec<f32> {
    let mut output = vec![99.0; frames * 2];
    mixer.fill(&mut output);
    output
}

fn assert_pcm(actual: &[f32], expected: &[[f32; 2]]) {
    assert_eq!(actual.len(), expected.len() * 2);
    for (index, (&actual, &expected)) in actual.iter().zip(expected.iter().flatten()).enumerate() {
        let expected = expected / i16::MAX as f32;
        assert!(
            (actual - expected).abs() < 0.00001,
            "sample {index}: {actual} != {expected}"
        );
    }
}

#[test]
fn chunked_fills_preserve_resampling_and_overlapping_voice_output() {
    let assets = Assets::new([
        ("sound/a.wav", pcm(1, 3, &[-3000, 9000, -10000, 5000])),
        (
            "sound/b.wav",
            pcm(2, 23, &[7000, -1000, -5000, 2000, 3000, -8000, 1000, 4000]),
        ),
    ]);
    let prepare = || {
        let mut mixer = mixer(b"1 sound/a.wav 3\n2 sound/b.wav 1\n", 17);
        mixer.play_looping(1, &assets).unwrap();
        mixer.play(1, &assets).unwrap();
        mixer.play(1, &assets).unwrap();
        mixer.play_looping(2, &assets).unwrap();
        mixer
    };
    let mut contiguous = prepare();
    let mut chunked = prepare();
    let expected = render(&mut contiguous, 47);
    let actual: Vec<_> = [0, 1, 2, 9, 1, 13, 0, 21]
        .into_iter()
        .flat_map(|frames| render(&mut chunked, frames))
        .collect();

    assert_eq!(actual, expected);
    for slot in [1, 2] {
        assert_eq!(
            chunked
                .bank
                .loaded(slot)
                .unwrap()
                .active_fixed()
                .collect::<Vec<_>>(),
            contiguous
                .bank
                .loaded(slot)
                .unwrap()
                .active_fixed()
                .collect::<Vec<_>>()
        );
    }
    assert_eq!(chunked.bank.loaded(1).unwrap().active_fixed().count(), 1);
}

#[test]
fn mixer_combines_mono_and_stereo_at_different_sample_rates() {
    let assets = Assets::new([
        ("sound/a.wav", pcm(1, 2, &[1000, 3000, 5000, 7000])),
        (
            "sound/b.wav",
            pcm(2, 8, &[4000, -4000, 10000, -10000, 6000, -6000, 0, 0]),
        ),
    ]);
    let mut mixer = mixer(b"1 sound/a.wav 1\n2 sound/b.wav 1\n", 4);
    mixer.play(1, &assets).unwrap();
    mixer.play(2, &assets).unwrap();

    assert_pcm(
        &render(&mut mixer, 4),
        &[
            [5000.0, -3000.0],
            [8000.0, -4000.0],
            [3000.0, 3000.0],
            [4000.0, 4000.0],
        ],
    );
    assert!(mixer.is_playing(1));
    assert!(!mixer.is_playing(2));
}

#[test]
fn saturated_polyphony_steals_the_seeded_voice_and_restarts_it() {
    let assets = Assets::new([("sound/a.wav", pcm(1, 8, &[1000; 8]))]);
    let mut mixer = mixer(b"1 sound/a.wav 3\n", 8);
    let sound = mixer.bank.sound(1, &assets).unwrap();

    for voice in 0..3 {
        assert_eq!(sound.play(false), voice);
        sound.advance_fixed(voice, (voice as u64 + 1) * FRAME);
    }
    assert_eq!(sound.play(true), 2);
    assert_eq!(
        sound.active_fixed().collect::<Vec<_>>(),
        [(0, FRAME, false), (1, 2 * FRAME, false), (2, 0, true)]
    );
    sound.advance_fixed(2, FRAME / 2);
    assert_eq!(sound.play(false), 2);
    assert_eq!(sound.play(true), 1);
    assert_eq!(sound.play(false), 1);
    assert_eq!(
        sound.active_fixed().collect::<Vec<_>>(),
        [(0, FRAME, false), (1, 0, false), (2, 0, false)]
    );
}

#[test]
fn looping_resampling_interpolates_toward_silence_before_wrapping() {
    let assets = Assets::new([("sound/a.wav", pcm(1, 2, &[8000, 16000]))]);
    let mut mixer = mixer(b"1 sound/a.wav 1\n", 4);
    mixer.play_looping(1, &assets).unwrap();

    assert_pcm(
        &render(&mut mixer, 8),
        &[
            [8000.0; 2],
            [12000.0; 2],
            [16000.0; 2],
            [8000.0; 2],
            [8000.0; 2],
            [12000.0; 2],
            [16000.0; 2],
            [8000.0; 2],
        ],
    );
    assert_eq!(
        mixer
            .bank
            .loaded(1)
            .unwrap()
            .active_fixed()
            .collect::<Vec<_>>(),
        [(0, 0, true)]
    );
}

#[test]
fn muted_and_inactive_effects_preserve_fractional_playback_positions() {
    let assets = Assets::new([("sound/a.wav", pcm(1, 3, &[1000, 3000, 5000]))]);
    let mut mixer = mixer(b"1 sound/a.wav 1\n", 4);
    mixer.play(1, &assets).unwrap();
    assert_pcm(&render(&mut mixer, 1), &[[1000.0; 2]]);

    mixer.bank.set_volume(volume::MIN);
    assert_eq!(render(&mut mixer, 7), vec![0.0; 14]);
    assert!(mixer.is_playing(1));
    assert_eq!(mixer.play(1, &assets), Err(Refusal::Muted));

    mixer.bank.set_volume(-2000);
    assert_pcm(&render(&mut mixer, 1), &[[250.0; 2]]);

    mixer.set_active(false);
    assert_eq!(render(&mut mixer, 9), vec![0.0; 18]);
    mixer.set_active(true);
    assert_pcm(&render(&mut mixer, 1), &[[400.0; 2]]);
    assert_eq!(assets.reads.get(), 1);
}

#[test]
fn attenuation_is_applied_before_overlapping_voices_are_clipped() {
    let assets = Assets::new([("sound/a.wav", pcm(2, 8, &[30000, -30000]))]);
    let mut mixer = mixer(b"1 sound/a.wav 2\n", 8);
    mixer.bank.set_volume(i32::MAX);
    assert_eq!(mixer.bank.volume(), volume::MAX);
    mixer.play(1, &assets).unwrap();
    mixer.play(1, &assets).unwrap();
    mixer.bank.set_volume(-2000);

    assert_pcm(&render(&mut mixer, 1), &[[6000.0, -6000.0]]);
    assert!(!mixer.is_playing(1));

    mixer.bank.set_volume(i32::MIN);
    assert_eq!(mixer.bank.volume(), volume::MIN);
}

#[test]
fn stopping_replays_cached_audio_while_releasing_reloads_and_stops_it() {
    let mut assets = Assets::new([("sound/a.wav", pcm(1, 8, &[1000, 2000]))]);
    let mut mixer = mixer(b"1 sound/a.wav 2\n", 8);
    mixer.play_looping(1, &assets).unwrap();
    assert_pcm(&render(&mut mixer, 1), &[[1000.0; 2]]);
    assets.files.insert("sound/a.wav", pcm(1, 8, &[7000, 9000]));

    mixer.stop(1);
    assert!(!mixer.is_playing(1));
    assert_eq!(render(&mut mixer, 3), vec![0.0; 6]);
    assert_eq!(mixer.play(1, &assets), Ok(0));
    assert_pcm(&render(&mut mixer, 1), &[[1000.0; 2]]);
    assert_eq!(assets.reads.get(), 1);

    mixer.bank.release(1);
    assert!(!mixer.is_playing(1));
    assert_eq!(render(&mut mixer, 3), vec![0.0; 6]);
    assert_eq!(mixer.play(1, &assets), Ok(0));
    assert_pcm(&render(&mut mixer, 1), &[[7000.0; 2]]);
    assert_eq!(assets.reads.get(), 2);
}

#[test]
fn loop_requests_during_a_one_shot_do_not_restart_or_extend_it() {
    let assets = Assets::new([("sound/a.wav", pcm(1, 8, &[1000, 2000]))]);
    let mut mixer = mixer(b"1 sound/a.wav 2\n", 8);
    mixer.play(1, &assets).unwrap();
    assert_pcm(&render(&mut mixer, 1), &[[1000.0; 2]]);

    mixer.play_looping(1, &assets).unwrap();
    assert_pcm(&render(&mut mixer, 2), &[[2000.0; 2], [0.0; 2]]);
    assert!(!mixer.is_playing(1));

    mixer.play_looping(1, &assets).unwrap();
    assert_pcm(
        &render(&mut mixer, 3),
        &[[1000.0; 2], [2000.0; 2], [1000.0; 2]],
    );
    assert!(mixer.is_playing(1));
}
