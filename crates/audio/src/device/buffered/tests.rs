use super::*;
use crate::{Bank, Loader, Track, volume};
use formats::audio::sound_list;
use std::sync::mpsc;
use std::time::Instant;

fn empty_mixer() -> Arc<Mutex<Mixer>> {
    Arc::new(Mutex::new(Mixer::new(
        Bank::new(sound_list::parse(b"").unwrap(), volume::MAX),
        volume::MAX,
        1000,
    )))
}

fn playing_mixer() -> Mixer {
    struct Pcm(Vec<u8>);
    impl Loader for Pcm {
        fn read(&self, _: &str) -> Option<Vec<u8>> {
            Some(self.0.clone())
        }
    }
    let samples: Vec<i16> = (0..64).map(|index| index * 100 - 3000).collect();
    let mut pcm = b"RIFF\xa4\0\0\0WAVEfmt \x10\0\0\0\x01\0\x01\0\xe8\x03\0\0\xd0\x07\0\0\x02\0\x10\0data\x80\0\0\0".to_vec();
    pcm.extend(samples.iter().flat_map(|sample| sample.to_le_bytes()));
    let loader = Pcm(pcm);
    let mut mixer = Mixer::new(
        Bank::new(
            sound_list::parse(b"1 sound/test.wav 1").unwrap(),
            volume::MAX,
        ),
        volume::MAX,
        1000,
    );
    mixer.play(1, &loader).unwrap();
    mixer
}

fn wait_until_full(consumer: &Consumer) {
    let deadline = Instant::now() + Duration::from_secs(2);
    while consumer.ring.remaining() != 0 && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(1));
    }
    assert_eq!(consumer.ring.remaining(), 0);
}

fn music_track() -> Track {
    Track::open(crate::music::behavior_tests::stereo_mp3(), "synthetic.mp3").unwrap()
}

fn music_mixer(rate: u32) -> Mixer {
    let mut mixer = playing_mixer();
    mixer.set_device_rate(rate);
    mixer.play_music(7, music_track());
    mixer
}

#[test]
fn queue_capacity_covers_large_callbacks_and_typical_twenty_millisecond_latency() {
    assert_eq!(capacity(48_000, 480), 960);
    assert_eq!(capacity(44_100, 441), 882);
    assert_eq!(capacity(48_000, 2048), 2144);
    assert_eq!(capacity(1000, 40), 42);
    assert_eq!(capacity(0, 0), 1);
}

#[test]
fn ring_preserves_fifo_frames_across_wraparound_and_underflow() {
    let ring = Ring::new(3);
    assert_eq!(ring.pop(), None);
    assert_eq!(ring.remaining(), 3);
    assert!(ring.push([1.0, -1.0]));
    assert!(ring.push([2.0, -2.0]));
    assert!(ring.push([3.0, -3.0]));
    assert!(!ring.push([99.0; 2]));
    assert_eq!(ring.remaining(), 0);
    assert_eq!(ring.pop(), Some([1.0, -1.0]));
    assert_eq!(ring.pop(), Some([2.0, -2.0]));
    assert!(ring.push([4.0, -4.0]));
    assert!(ring.push([5.0, -5.0]));
    for expected in [3.0, 4.0, 5.0] {
        assert_eq!(ring.pop(), Some([expected, -expected]));
    }
    assert_eq!(ring.pop(), None);
    assert_eq!(ring.remaining(), 3);
}

#[test]
fn ring_preserves_every_frame_between_concurrent_producer_and_consumer() {
    let ring = Arc::new(Ring::new(17));
    let producer_ring = Arc::clone(&ring);
    let producer = thread::spawn(move || {
        for sample in 0..10_000 {
            while !producer_ring.push([sample as f32, -(sample as f32)]) {
                thread::yield_now();
            }
        }
    });
    for sample in 0..10_000 {
        let frame = loop {
            if let Some(frame) = ring.pop() {
                break frame;
            }
            thread::yield_now();
        };
        assert_eq!(frame, [sample as f32, -(sample as f32)]);
    }
    producer.join().unwrap();
    assert_eq!(ring.pop(), None);
}

#[test]
fn output_maps_mono_stereo_and_surround_and_silences_underruns() {
    for channels in [1, 2, 6] {
        let ring = Arc::new(Ring::new(2));
        ring.push([0.75, -0.25]);
        ring.push([-0.5, 1.0]);
        let consumer = Consumer { ring };
        let mut output = vec![99.0; channels * 3];
        consumer.fill(&mut output, channels, |sample| sample);
        let mut expected = vec![0.0; channels * 3];
        if channels == 1 {
            expected[..2].copy_from_slice(&[0.25, 0.25]);
        } else {
            expected[..2].copy_from_slice(&[0.75, -0.25]);
            expected[channels..channels + 2].copy_from_slice(&[-0.5, 1.0]);
        }
        assert_eq!(output, expected);
        consumer.fill(&mut output, channels, |sample| sample);
        assert_eq!(output, vec![0.0; channels * 3]);
    }
}

#[test]
fn output_converts_i16_and_leaves_incomplete_frames_silent() {
    let ring = Arc::new(Ring::new(3));
    ring.push([1.0, -1.0]);
    ring.push([0.5, -0.5]);
    ring.push([0.25, -0.25]);
    let consumer = Consumer { ring };
    let mut output = [99; 5];
    consumer.fill(&mut output, 2, |sample| (sample * i16::MAX as f32) as i16);
    assert_eq!(output, [32767, -32767, 16383, -16383, 0]);
    let mut next = [99; 2];
    consumer.fill(&mut next, 2, |sample| (sample * i16::MAX as f32) as i16);
    assert_eq!(next, [8191, -8191]);
}

#[test]
fn zero_channels_and_empty_callbacks_do_not_consume_audio() {
    let ring = Arc::new(Ring::new(1));
    ring.push([0.5, -0.5]);
    let consumer = Consumer { ring };
    let mut output = [99.0; 4];
    consumer.fill(&mut output, 0, |sample| sample);
    assert_eq!(output, [0.0; 4]);
    consumer.fill(&mut [], 2, |sample| sample);
    consumer.fill(&mut output, 2, |sample| sample);
    assert_eq!(output, [0.5, -0.5, 0.0, 0.0]);
}

#[test]
fn producer_stops_even_when_the_mixer_is_locked() {
    let mixer = empty_mixer();
    let guard = mixer.lock().unwrap();
    let (producer, _consumer) = Producer::spawn(Arc::clone(&mixer), 1000, 0).unwrap();
    let (done, stopped) = mpsc::channel();
    let stopping = thread::spawn(move || {
        drop(producer);
        done.send(()).unwrap();
    });
    let result = stopped.recv_timeout(Duration::from_secs(2));
    drop(guard);
    stopping.join().unwrap();
    assert!(result.is_ok());
    assert_eq!(Arc::strong_count(&mixer), 1);
}

#[test]
fn producer_bounds_queued_audio_and_releases_the_mixer_on_stop() {
    let mixer = empty_mixer();
    let (mut producer, consumer) = Producer::spawn(Arc::clone(&mixer), 1000, 0).unwrap();
    assert_eq!(consumer.ring.frames.len() - 1, 20);
    wait_until_full(&consumer);
    producer.stop();
    producer.stop();
    assert_eq!(Arc::strong_count(&mixer), 1);
    let mut frames = 0;
    while let Some(frame) = consumer.ring.pop() {
        assert_eq!(frame, [0.0; 2]);
        frames += 1;
    }
    assert_eq!(frames, 20);
}

#[test]
fn producer_audio_matches_direct_mixing_without_a_device() {
    let mut expected = [0.0; 40];
    playing_mixer().fill(&mut expected);
    let mixer = Arc::new(Mutex::new(playing_mixer()));
    let (mut producer, consumer) = Producer::spawn(mixer, 1000, 0).unwrap();
    wait_until_full(&consumer);
    producer.stop();
    let mut output = [99.0; 40];
    consumer.fill(&mut output, 2, |sample| sample);
    assert_eq!(output, expected);
}

#[test]
fn producer_fills_a_device_period_larger_than_twenty_milliseconds() {
    let mut expected = [0.0; 80];
    playing_mixer().fill(&mut expected);
    let mixer = Arc::new(Mutex::new(playing_mixer()));
    let (mut producer, consumer) = Producer::spawn(mixer, 1000, 40).unwrap();
    wait_until_full(&consumer);
    producer.stop();
    let mut output = [99.0; 80];
    consumer.fill(&mut output, 2, |sample| sample);
    assert_eq!(output, expected);
}

#[test]
fn pause_mute_stop_and_volume_changes_apply_after_the_queued_audio() {
    for control in 0..4 {
        let mut direct = playing_mixer();
        let mut queued = [0.0; 40];
        direct.fill(&mut queued);
        let mixer = Arc::new(Mutex::new(playing_mixer()));
        let (mut producer, consumer) = Producer::spawn(Arc::clone(&mixer), 1000, 0).unwrap();
        wait_until_full(&consumer);
        {
            let mut mixer = mixer.lock().unwrap();
            for target in [&mut *mixer, &mut direct] {
                match control {
                    0 => target.set_active(false),
                    1 => target.bank.set_volume(volume::MIN),
                    2 => target.stop(1),
                    _ => target.bank.set_volume(-2000),
                }
            }
        }

        let mut output = [99.0; 40];
        consumer.fill(&mut output, 2, |sample| sample);
        assert_eq!(output, queued);
        wait_until_full(&consumer);
        producer.stop();

        let mut expected = [0.0; 40];
        direct.fill(&mut expected);
        consumer.fill(&mut output, 2, |sample| sample);
        assert_eq!(output, expected);
        let mut mixer = mixer.lock().unwrap();
        assert_eq!(
            mixer
                .bank
                .loaded(1)
                .unwrap()
                .active_fixed()
                .collect::<Vec<_>>(),
            direct
                .bank
                .loaded(1)
                .unwrap()
                .active_fixed()
                .collect::<Vec<_>>()
        );
        for target in [&mut *mixer, &mut direct] {
            target.set_active(true);
            target.bank.set_volume(volume::MAX);
        }
        mixer.fill(&mut output);
        direct.fill(&mut expected);
        assert_eq!(output, expected);
    }
}

#[test]
fn music_blocks_match_direct_output_at_different_rates_volumes_and_loop_boundaries() {
    for rate in [4000, 8000, 11025, 48000] {
        for volume in [volume::MAX, volume::MIN, -2000] {
            let mut direct = music_mixer(rate);
            direct.set_music_volume(volume);
            let mut buffered = music_mixer(rate);
            buffered.set_music_volume(volume);
            let buffered = Mutex::new(buffered);
            for frames in [0, 1, 3, 16, 93, 257, 1024, 3000] {
                let mut expected = vec![0.0; frames * CHANNELS];
                direct.fill(&mut expected);
                let mut actual = vec![99.0; frames * CHANNELS];
                assert!(mix_block(&buffered, &mut actual, || false));
                assert_eq!(
                    actual, expected,
                    "rate {rate}, volume {volume}, frames {frames}"
                );
            }
            assert_eq!(buffered.lock().unwrap().music_slot(), direct.music_slot());
        }
    }
}

#[test]
fn music_decoding_keeps_the_mixer_available_to_controls() {
    let mixer = Mutex::new(music_mixer(8000));
    let mut output = [99.0; 128];
    let mut checks = 0;
    assert!(mix_block(&mixer, &mut output, || {
        checks += 1;
        assert!(mixer.try_lock().is_ok());
        false
    }));
    assert!(checks >= 4);
    let mut expected = [0.0; 128];
    music_mixer(8000).fill(&mut expected);
    assert_eq!(output, expected);
}

#[test]
fn stopping_or_replacing_detached_music_cannot_restore_the_old_track() {
    for replace in [false, true] {
        let mixer = Mutex::new(music_mixer(8000));
        let mut direct = music_mixer(8000);
        if replace {
            direct.play_music(11, music_track());
        } else {
            direct.stop_music();
        }
        let mut replacement = replace.then(music_track);
        let mut checks = 0;
        let mut output = [99.0; 128];
        assert!(mix_block(&mixer, &mut output, || {
            checks += 1;
            if checks == 2 {
                let mut mixer = mixer.try_lock().unwrap();
                if let Some(track) = replacement.take() {
                    mixer.play_music(11, track);
                } else {
                    mixer.stop_music();
                }
            }
            false
        }));
        let mut expected = [0.0; 128];
        direct.fill(&mut expected);
        assert_eq!(output, expected);
        assert_eq!(mixer.lock().unwrap().music_slot(), direct.music_slot());
    }
}

#[test]
fn a_fully_buffered_music_job_is_rejected_after_a_later_switch() {
    let mut mixer = music_mixer(11025);
    let mut old = mixer.take_music_buffer(257).unwrap();
    assert!(old.buffer(|| false));
    mixer.play_music(11, music_track());
    assert!(mixer.restore_music_buffer(old).is_some());
    assert_eq!(mixer.music_slot(), Some(11));
    let mut expected = [0.0; 514];
    let mut reference = music_mixer(11025);
    reference.play_music(11, music_track());
    reference.fill(&mut expected);
    let mut actual = [99.0; 514];
    assert!(mix_block(&Mutex::new(mixer), &mut actual, || false));
    assert_eq!(actual, expected);
}

#[test]
fn pausing_during_music_buffering_preserves_the_fractional_resume_position() {
    let mut direct = music_mixer(11025);
    let mut buffered = music_mixer(11025);
    let mut warmup = [0.0; 1802];
    direct.fill(&mut warmup);
    buffered.fill(&mut warmup);
    let mixer = Mutex::new(buffered);
    let mut checks = 0;
    let mut output = [99.0; 514];
    assert!(mix_block(&mixer, &mut output, || {
        checks += 1;
        if checks == 2 {
            mixer.try_lock().unwrap().set_active(false);
        }
        false
    }));
    assert_eq!(output, [0.0; 514]);
    assert_eq!(mixer.lock().unwrap().music_slot(), Some(7));
    mixer.lock().unwrap().set_active(true);
    assert!(mix_block(&mixer, &mut output, || false));
    let mut expected = [0.0; 514];
    direct.fill(&mut expected);
    assert_eq!(output, expected);
}

#[test]
fn a_buffered_track_survives_contention_while_waiting_to_reattach() {
    let mixer = Mutex::new(music_mixer(8000));
    let mut held = None;
    let mut checks = 0;
    let mut output = [99.0; 128];
    assert!(mix_block(&mixer, &mut output, || {
        checks += 1;
        if checks == 3 {
            held = Some(mixer.try_lock().unwrap());
        } else if checks == 5 {
            held = None;
        }
        false
    }));
    assert_eq!(checks, 5);
    let mut expected = [0.0; 128];
    music_mixer(8000).fill(&mut expected);
    assert_eq!(output, expected);
}

#[test]
fn cancellation_does_not_wait_for_a_mixer_lock_while_music_is_detached() {
    let mixer = Mutex::new(music_mixer(8000));
    let mut held = None;
    let mut checks = 0;
    assert!(!mix_block(&mixer, &mut [0.0; 128], || {
        checks += 1;
        if checks == 3 {
            held = Some(mixer.try_lock().unwrap());
        }
        checks >= 4
    }));
    assert_eq!(checks, 4);
    drop(held);
}

#[test]
fn stopping_music_interrupts_a_corrupt_prefix_before_decoding_the_whole_file() {
    let valid = crate::music::behavior_tests::stereo_mp3();
    let mut corrupt = valid[..216].to_vec();
    corrupt[4..21].fill(0xff);
    let mut bytes = corrupt.repeat(600);
    bytes.extend_from_slice(&valid);
    let mut source = playing_mixer();
    source.play_music(7, Track::open(bytes, "corrupt-prefix.mp3").unwrap());
    let mixer = Mutex::new(source);
    let mut checks = 0;
    let mut output = [99.0; 128];
    assert!(mix_block(&mixer, &mut output, || {
        checks += 1;
        if checks == 7 {
            mixer.try_lock().unwrap().stop_music();
        }
        false
    }));
    assert_eq!(checks, 8);
    assert_eq!(mixer.lock().unwrap().music_slot(), None);
    let mut expected = [0.0; 128];
    playing_mixer().fill(&mut expected);
    assert_eq!(output, expected);
}

#[test]
fn producer_music_matches_direct_mixing_without_a_device() {
    let rate = 11025;
    let frames = capacity(rate, 0);
    let mut expected = vec![0.0; frames * CHANNELS];
    music_mixer(rate).fill(&mut expected);
    let mixer = Arc::new(Mutex::new(music_mixer(rate)));
    let (mut producer, consumer) = Producer::spawn(mixer, rate, 0).unwrap();
    wait_until_full(&consumer);
    producer.stop();
    let mut output = vec![99.0; frames * CHANNELS];
    consumer.fill(&mut output, 2, |sample| sample);
    assert_eq!(output, expected);
}
