use super::*;
use crate::scene::skill_anim::{DamageSoundRequest, PulsePlan, PulseSide, State};

fn wave() -> Vec<u8> {
    let samples = [1000_i16, 2000, 3000, 4000, 5000, 6000, 7000, 8000];
    let data_len = (samples.len() * 2) as u32;
    let rate = audio::DEFAULT_RATE;
    let mut bytes = b"RIFF".to_vec();
    bytes.extend_from_slice(&(36 + data_len).to_le_bytes());
    bytes.extend_from_slice(b"WAVEfmt ");
    bytes.extend_from_slice(&16_u32.to_le_bytes());
    bytes.extend_from_slice(&1_u16.to_le_bytes());
    bytes.extend_from_slice(&1_u16.to_le_bytes());
    bytes.extend_from_slice(&rate.to_le_bytes());
    bytes.extend_from_slice(&(rate * 2).to_le_bytes());
    bytes.extend_from_slice(&2_u16.to_le_bytes());
    bytes.extend_from_slice(&16_u16.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&data_len.to_le_bytes());
    bytes.extend(samples.into_iter().flat_map(i16::to_le_bytes));
    bytes
}

fn scene_audio() -> SceneAudio {
    let sound_list = b"1 sound/tone.wav 3\n6 sound/tone.wav 2\n21 sound/tone.wav 3\n24 sound/tone.wav 2\n25 sound/missing.wav 1\n26 sound/bad.wav 1\n101 sound/tone.wav 2\n102 sound/tone.wav 2\n107 sound/tone.wav 2\n";
    let game = GameDir::from_files(
        "synthetic-scene-audio",
        [
            ("sound/soundlist.txt".into(), sound_list.to_vec()),
            ("sound/tone.wav".into(), wave()),
            ("sound/bad.wav".into(), b"not a waveform".to_vec()),
        ],
    )
    .unwrap();
    SceneAudio::open(&game, &Config::default(), true)
}

fn active(audio: &SceneAudio, slot: i64) -> Vec<(usize, bool)> {
    audio
        .engine
        .mixer()
        .bank
        .loaded(slot)
        .map(|sound| sound.active().collect())
        .unwrap_or_default()
}

fn damage(slot: i64, now_ms: u32) -> DamageSoundRequest {
    DamageSoundRequest {
        entity: 7,
        token: 1,
        slot,
        kind: 0,
        previous_kind: 0,
        last_play_ms: 100,
        now_ms,
    }
}

#[test]
fn once_suppresses_overlap_and_unconditional_allocates_another_voice() {
    let audio = scene_audio();
    audio.play_once(1);
    audio.engine.mixer().fill(&mut [0.0; 4]);
    audio.play_once(1);
    assert_eq!(active(&audio, 1), [(2, false)]);

    audio.play_unconditional(UnconditionalSound::new(1));
    assert_eq!(active(&audio, 1), [(2, false), (0, false)]);

    audio.engine.mixer().fill(&mut [0.0; 16]);
    assert!(!audio.engine.is_playing(1));
    audio.play_once(1);
    assert_eq!(active(&audio, 1), [(0, false)]);
}

#[test]
fn damage_uses_existing_voice_kind_and_strict_interval_boundary() {
    let audio = scene_audio();
    assert_eq!(audio.play_damage(damage(21, 100)), Some(true));
    assert_eq!(audio.play_damage(damage(21, 500)), Some(false));
    assert_eq!(active(&audio, 21).len(), 1);
    assert_eq!(audio.play_damage(damage(21, 501)), Some(true));

    let changed_kind = DamageSoundRequest {
        kind: 1,
        ..damage(21, 100)
    };
    assert_eq!(audio.play_damage(changed_kind), Some(true));
    assert_eq!(active(&audio, 21).len(), 3);
}

#[test]
fn damage_does_not_report_success_for_suppressed_missing_invalid_or_muted_audio() {
    let audio = scene_audio();
    for slot in [24, 25, 26, 0, 511] {
        assert_eq!(
            audio.play_damage(damage(slot, 1000)),
            Some(false),
            "slot {slot}"
        );
        assert!(!audio.engine.is_playing(slot));
    }

    audio.set_volumes(0, 100);
    assert_eq!(audio.play_damage(damage(21, 1000)), Some(false));
    assert!(!audio.engine.is_playing(21));

    audio.set_volumes(100, 100);
    assert_eq!(audio.play_damage(damage(21, 1000)), Some(true));
}

#[test]
fn damage_feedback_updates_only_the_current_request_after_success() {
    let audio = scene_audio();
    let mut state = State::default();
    let plan = PulsePlan {
        side: PulseSide::Main,
        selector: 1,
        exits_function: false,
        clock_write: None,
        steps: Vec::new(),
    };
    state.queue_damage_sound(7, 0, 0, &plan, 100);
    let first = state.take_damage_sounds()[0];
    let played = audio.play_damage(first).unwrap();
    assert!(state.finish_damage_sound(first, played));
    assert_eq!(state.damage_sound_state(7), (0, 100));

    state.queue_damage_sound(7, 0, 0, &plan, 200);
    let throttled = state.take_damage_sounds()[0];
    assert_eq!(audio.play_damage(throttled), Some(false));
    assert!(state.finish_damage_sound(throttled, false));
    assert_eq!(state.damage_sound_state(7), (0, 100));

    state.queue_damage_sound(7, 0, 0, &plan, 600);
    let stale = state.take_damage_sounds()[0];
    state.queue_damage_sound(7, 0, 0, &plan, 700);
    let current = state.take_damage_sounds()[0];
    assert!(!state.finish_damage_sound(stale, true));
    assert!(state.finish_damage_sound(current, true));
    assert_eq!(state.damage_sound_state(7), (0, 700));

    state.queue_damage_sound(7, 0, 0, &plan, 800);
    let forgotten = state.take_damage_sounds()[0];
    state.forget(7);
    assert!(!state.finish_damage_sound(forgotten, true));
    assert_eq!(state.damage_sound_state(7), (-1, 0));
}

#[test]
fn lifecycle_stops_only_the_designated_ambience_slots() {
    let mut audio = scene_audio();
    audio.engine.play_looping(AMBIENCE_STOPPED, &audio.assets);
    audio.play_once(1);
    audio.enter();
    assert_eq!(active(&audio, AMBIENCE), [(0, true)]);
    assert!(!audio.engine.is_playing(AMBIENCE_STOPPED));
    assert!(audio.engine.is_playing(1));

    audio.engine.mixer().fill(&mut [0.0; 4]);
    audio.enter();
    assert_eq!(active(&audio, AMBIENCE), [(2, true)]);
    audio.leave();
    assert!(!audio.engine.is_playing(AMBIENCE));
    assert!(audio.engine.is_playing(1));

    for slot in [6, 107] {
        audio.engine.play_looping(slot, &audio.assets);
    }
    audio.leave_field();
    assert!(!audio.engine.is_playing(6));
    assert!(!audio.engine.is_playing(107));
    assert!(audio.engine.is_playing(1));
}

#[test]
fn focus_pause_preserves_voices_and_runtime_zero_volume_mutes() {
    let audio = scene_audio();
    audio.play_once(1);
    audio.engine.mixer().fill(&mut [0.0; 4]);
    let before = active(&audio, 1);

    audio.set_active(false);
    let mut output = [1.0; 10];
    audio.engine.mixer().fill(&mut output);
    assert_eq!(output, [0.0; 10]);
    assert_eq!(active(&audio, 1), before);

    audio.set_active(true);
    audio.set_volumes(0, 0);
    audio.engine.mixer().fill(&mut output);
    assert_eq!(output, [0.0; 10]);
    assert_eq!(active(&audio, 1), before);

    audio.set_volumes(255, 255);
    assert_eq!(audio.engine.mixer().bank.volume(), audio::volume::MAX);
    assert_eq!(audio.engine.mixer().music_volume(), audio::volume::MAX);
    audio.engine.mixer().fill(&mut output);
    assert!(output.iter().any(|sample| *sample != 0.0));
}

#[test]
fn missing_music_is_reported_without_preventing_ambience() {
    let mut audio = scene_audio();
    assert!(!audio.switch_music(music::SLOTS));
    assert!(!audio.switch_music(1));
    assert!(audio.engine.mixer().music_slot().is_none());

    audio.enter();
    assert!(audio.notes.iter().any(|note| note.contains("music slot 0")));
    assert!(audio.engine.is_playing(AMBIENCE));
}

#[cfg(not(target_arch = "wasm32"))]
mod deferred {
    use super::*;
    use std::sync::mpsc::{Receiver, Sender, channel};
    use std::time::{Duration, Instant};

    struct ControlledAssets {
        assets: Assets,
        started: Sender<String>,
        resume: Receiver<()>,
    }

    impl Loader for ControlledAssets {
        fn read(&self, path: &str) -> Option<Vec<u8>> {
            self.started.send(path.to_owned()).unwrap();
            self.resume.recv().unwrap();
            self.assets.read(path)
        }
    }

    fn deferred_audio() -> (SceneAudio, Receiver<String>, Sender<()>) {
        let mut audio = scene_audio();
        let (started_tx, started_rx) = channel();
        let (resume_tx, resume_rx) = channel();
        let loader = ControlledAssets {
            assets: audio.assets.clone(),
            started: started_tx,
            resume: resume_rx,
        };
        let sounds = audio.engine.mixer().bank.list().clone();
        audio.engine = Engine::silent(sounds, music::MusicTable::default(), 100, 100)
            .with_background_loader(loader);
        (audio, started_rx, resume_tx)
    }

    fn results(audio: &SceneAudio, count: usize) -> Vec<(DamageSoundRequest, bool)> {
        let until = Instant::now() + Duration::from_secs(5);
        let mut results = Vec::new();
        while results.len() < count {
            results.extend(audio.take_damage_results());
            assert!(Instant::now() < until, "audio results did not become ready");
            std::thread::yield_now();
        }
        results
    }

    fn queue(state: &mut State, side: PulseSide, now_ms: u32) -> DamageSoundRequest {
        state.queue_damage_sound(
            7,
            0,
            0,
            &PulsePlan {
                side,
                selector: 1,
                exits_function: false,
                clock_write: None,
                steps: Vec::new(),
            },
            now_ms,
        );
        state.take_damage_sounds()[0]
    }

    #[test]
    fn pending_damage_reports_actual_success_with_the_original_request_time() {
        let (audio, started, resume) = deferred_audio();
        let request = damage(21, 1000);
        assert_eq!(audio.play_damage(request), None);
        assert_eq!(
            started.recv_timeout(Duration::from_secs(5)).unwrap(),
            "sound/tone.wav"
        );
        assert!(!audio.engine.is_playing(21));
        assert!(audio.take_damage_results().is_empty());
        resume.send(()).unwrap();
        assert_eq!(results(&audio, 1), [(request, true)]);
        assert!(audio.engine.is_playing(21));
        assert!(audio.take_damage_results().is_empty());
    }

    #[test]
    fn pending_damage_reports_missing_files_as_failure() {
        let (audio, started, resume) = deferred_audio();
        let request = damage(25, 1000);
        assert_eq!(audio.play_damage(request), None);
        assert_eq!(
            started.recv_timeout(Duration::from_secs(5)).unwrap(),
            "sound/missing.wav"
        );
        resume.send(()).unwrap();
        assert_eq!(results(&audio, 1), [(request, false)]);
        assert_eq!(audio.play_damage(request), Some(false));
        assert!(!audio.engine.is_playing(25));
    }

    #[test]
    fn scene_cancellation_prevents_pending_damage_from_playing_or_completing_in_the_next_scene() {
        let (audio, started, resume) = deferred_audio();
        let old = damage(21, 1000);
        assert_eq!(audio.play_damage(old), None);
        started.recv_timeout(Duration::from_secs(5)).unwrap();
        audio.cancel_pending_damage();
        let next = DamageSoundRequest {
            now_ms: 2000,
            ..old
        };
        assert_eq!(audio.play_damage(next), None);
        resume.send(()).unwrap();
        assert_eq!(results(&audio, 1), [(next, true)]);
        assert_eq!(active(&audio, 21).len(), 1);
        assert!(audio.take_damage_results().is_empty());
    }

    #[test]
    fn muting_before_a_pending_load_finishes_prevents_damage_success() {
        let (audio, started, resume) = deferred_audio();
        let request = damage(21, 1000);
        assert_eq!(audio.play_damage(request), None);
        started.recv_timeout(Duration::from_secs(5)).unwrap();
        audio.set_volumes(0, 100);
        resume.send(()).unwrap();
        assert_eq!(results(&audio, 1), [(request, false)]);
        assert!(!audio.engine.is_playing(21));
    }

    #[test]
    fn a_later_suppressed_request_retains_the_first_successful_damage_timestamp() {
        for ready_before_next_frame in [false, true] {
            let (audio, started, resume) = deferred_audio();
            let mut state = State::default();
            let first = queue(&mut state, PulseSide::Main, 100);
            assert_eq!(audio.play_damage(first), None);
            started.recv_timeout(Duration::from_secs(5)).unwrap();

            if ready_before_next_frame {
                audio.engine.preload(6, &audio.assets);
                resume.send(()).unwrap();
                started.recv_timeout(Duration::from_secs(5)).unwrap();
            }

            let latest = queue(&mut state, PulseSide::Main, 200);
            assert_eq!(audio.play_damage(latest), None);
            assert_eq!(state.damage_sound_state(7), (0, 0));
            resume.send(()).unwrap();

            let completed = results(&audio, 1);
            assert_eq!(
                completed,
                [(
                    DamageSoundRequest {
                        now_ms: 100,
                        ..latest
                    },
                    true
                )]
            );
            assert!(state.finish_damage_sound(completed[0].0, completed[0].1));
            assert_eq!(state.damage_sound_state(7), (0, 100));
            assert_eq!(active(&audio, 21).len(), 1);
        }
    }

    #[test]
    fn overlapping_deferred_damage_uses_the_last_successful_request_timestamp() {
        let (audio, started, resume) = deferred_audio();
        let mut state = State::default();
        let first = queue(&mut state, PulseSide::Main, 100);
        assert_eq!(audio.play_damage(first), None);
        started.recv_timeout(Duration::from_secs(5)).unwrap();
        let next = queue(&mut state, PulseSide::Off, 200);
        assert_eq!(audio.play_damage(next), None);
        resume.send(()).unwrap();

        let completed = results(&audio, 1);
        assert_eq!(completed, [(next, true)]);
        assert!(state.finish_damage_sound(completed[0].0, completed[0].1));
        assert_eq!(state.damage_sound_state(7), (1, 200));
        assert_eq!(active(&audio, 21).len(), 2);
    }

    #[test]
    fn a_later_cached_sound_waits_for_the_prior_request_and_keeps_its_timestamp() {
        let (audio, started, resume) = deferred_audio();
        let warm = damage(1, 10);
        assert_eq!(audio.play_damage(warm), None);
        started.recv_timeout(Duration::from_secs(5)).unwrap();
        resume.send(()).unwrap();
        assert_eq!(results(&audio, 1), [(warm, true)]);
        audio.engine.stop(1);

        let first = damage(21, 100);
        assert_eq!(audio.play_damage(first), None);
        started.recv_timeout(Duration::from_secs(5)).unwrap();
        let latest = DamageSoundRequest {
            token: 2,
            kind: 1,
            ..damage(1, 200)
        };
        assert_eq!(audio.play_damage(latest), None);
        assert!(!audio.engine.is_playing(1));
        resume.send(()).unwrap();
        assert_eq!(results(&audio, 1), [(latest, true)]);
        assert!(audio.engine.is_playing(1));
    }

    #[test]
    fn a_reused_actor_id_cannot_inherit_a_forgotten_actors_pending_damage() {
        let (audio, started, resume) = deferred_audio();
        let mut state = State::default();
        let first = queue(&mut state, PulseSide::Main, 100);
        assert_eq!(audio.play_damage(first), None);
        started.recv_timeout(Duration::from_secs(5)).unwrap();
        state.forget(7);
        let next = queue(&mut state, PulseSide::Main, 200);
        assert_eq!(next.previous_kind, -1);
        assert_eq!(audio.play_damage(next), None);
        resume.send(()).unwrap();

        let completed = results(&audio, 1);
        assert_eq!(completed, [(next, true)]);
        assert!(state.finish_damage_sound(completed[0].0, completed[0].1));
        assert_eq!(state.damage_sound_state(7), (0, 200));
        assert_eq!(active(&audio, 21).len(), 1);
    }

    #[test]
    fn leaving_a_scene_cancels_pending_ordinary_sound_requests() {
        let (audio, started, resume) = deferred_audio();
        audio.play_once(1);
        audio.play_unconditional(UnconditionalSound::new(1));
        started.recv_timeout(Duration::from_secs(5)).unwrap();
        audio.cancel_pending_sounds();
        let next = damage(21, 1000);
        assert_eq!(audio.play_damage(next), None);
        resume.send(()).unwrap();
        started.recv_timeout(Duration::from_secs(5)).unwrap();
        resume.send(()).unwrap();
        assert_eq!(results(&audio, 1), [(next, true)]);
        assert!(!audio.engine.is_playing(1));
    }

    #[test]
    fn pending_damage_completed_while_unfocused_starts_paused_and_resumes_in_place() {
        let (audio, started, resume) = deferred_audio();
        let request = damage(21, 1000);
        assert_eq!(audio.play_damage(request), None);
        started.recv_timeout(Duration::from_secs(5)).unwrap();
        audio.set_active(false);
        resume.send(()).unwrap();
        assert_eq!(results(&audio, 1), [(request, true)]);
        let mut output = [1.0; 4];
        audio.engine.mixer().fill(&mut output);
        assert_eq!(output, [0.0; 4]);
        assert_eq!(active(&audio, 21), [(0, false)]);
        audio.set_active(true);
        audio.engine.mixer().fill(&mut output);
        assert!(output.iter().any(|&sample| sample != 0.0));
        assert_eq!(active(&audio, 21), [(2, false)]);
    }

    #[test]
    fn damage_throttling_matches_synchronous_playback_across_frames_and_within_a_frame() {
        for same_frame in [false, true] {
            let synchronous = scene_audio();
            let mut synchronous_state = State::default();
            let first = queue(&mut synchronous_state, PulseSide::Main, 5000);
            let latest = if same_frame {
                let latest = queue(&mut synchronous_state, PulseSide::Main, 5000);
                let played = synchronous.play_damage(first).unwrap();
                synchronous_state.finish_damage_sound(first, played);
                latest
            } else {
                let played = synchronous.play_damage(first).unwrap();
                synchronous_state.finish_damage_sound(first, played);
                queue(&mut synchronous_state, PulseSide::Main, 5050)
            };
            let played = synchronous.play_damage(latest).unwrap();
            synchronous_state.finish_damage_sound(latest, played);

            let (deferred, started, resume) = deferred_audio();
            let mut deferred_state = State::default();
            let first = queue(&mut deferred_state, PulseSide::Main, 5000);
            assert_eq!(deferred.play_damage(first), None);
            started.recv_timeout(Duration::from_secs(5)).unwrap();
            let latest = queue(
                &mut deferred_state,
                PulseSide::Main,
                if same_frame { 5000 } else { 5050 },
            );
            assert_eq!(deferred.play_damage(latest), None);
            resume.send(()).unwrap();
            let completed = results(&deferred, 1);
            deferred_state.finish_damage_sound(completed[0].0, completed[0].1);

            assert_eq!(active(&deferred, 21), active(&synchronous, 21));
            assert_eq!(
                deferred_state.damage_sound_state(7),
                synchronous_state.damage_sound_state(7)
            );
        }
    }

    #[test]
    fn a_failed_pending_sound_does_not_rate_limit_the_next_valid_sound() {
        let (audio, started, resume) = deferred_audio();
        let warm = DamageSoundRequest {
            entity: 9,
            ..damage(21, 1)
        };
        assert_eq!(audio.play_damage(warm), None);
        started.recv_timeout(Duration::from_secs(5)).unwrap();
        resume.send(()).unwrap();
        assert_eq!(results(&audio, 1), [(warm, true)]);

        let missing = DamageSoundRequest {
            last_play_ms: 0,
            previous_kind: -1,
            ..damage(25, 5000)
        };
        assert_eq!(audio.play_damage(missing), None);
        started.recv_timeout(Duration::from_secs(5)).unwrap();
        let next = DamageSoundRequest {
            token: 2,
            last_play_ms: 0,
            ..damage(21, 5050)
        };
        assert_eq!(audio.play_damage(next), None);
        resume.send(()).unwrap();
        assert_eq!(results(&audio, 1), [(next, true)]);
        assert_eq!(active(&audio, 21).len(), 2);
    }

    #[test]
    fn damage_requested_while_muted_cannot_start_after_a_prior_load_finishes() {
        let (audio, started, resume) = deferred_audio();
        let first = damage(21, 100);
        assert_eq!(audio.play_damage(first), None);
        started.recv_timeout(Duration::from_secs(5)).unwrap();
        audio.set_volumes(0, 100);
        let muted = DamageSoundRequest {
            token: 2,
            kind: 1,
            ..damage(1, 200)
        };
        assert_eq!(audio.play_damage(muted), None);
        audio.set_volumes(100, 100);
        resume.send(()).unwrap();
        assert_eq!(
            results(&audio, 1),
            [(
                DamageSoundRequest {
                    now_ms: 100,
                    ..muted
                },
                true
            )]
        );
        assert!(audio.engine.is_playing(21));
        assert!(!audio.engine.is_playing(1));
    }

    #[test]
    fn pending_damage_is_bounded_when_a_loader_stalls() {
        let (audio, started, resume) = deferred_audio();
        assert_eq!(audio.play_damage(damage(21, 100)), None);
        started.recv_timeout(Duration::from_secs(5)).unwrap();
        for token in 2..(MAX_PENDING_DAMAGE as u64 + 10) {
            assert_eq!(
                audio.play_damage(DamageSoundRequest {
                    token,
                    ..damage(21, 200)
                }),
                None
            );
        }
        let pending = audio.pending_damage.borrow();
        assert_eq!(
            pending.receipts.len()
                + pending
                    .feedback
                    .values()
                    .map(|feedback| feedback.queued.len())
                    .sum::<usize>(),
            MAX_PENDING_DAMAGE
        );
        drop(pending);
        audio.cancel_pending_sounds();
        resume.send(()).unwrap();
        assert!(audio.pending_damage.borrow().receipts.is_empty());
        assert!(audio.pending_damage.borrow().feedback.is_empty());
    }
}
