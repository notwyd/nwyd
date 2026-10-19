use super::*;
use crate::{Engine, PlayResult, volume};
use formats::audio::sound_list;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread::{self, ThreadId};
use std::time::{Duration, Instant};

const WAIT: Duration = Duration::from_secs(5);

enum Reply {
    Bytes(Option<Vec<u8>>),
    Panic,
}

struct Read {
    path: String,
    thread: ThreadId,
    name: Option<String>,
    reply: Sender<Reply>,
}

impl Read {
    fn respond(self, bytes: Option<Vec<u8>>) {
        self.reply.send(Reply::Bytes(bytes)).unwrap();
    }
}

struct ControlledLoader {
    reads: Sender<Read>,
}

impl Loader for ControlledLoader {
    fn read(&self, path: &str) -> Option<Vec<u8>> {
        let (reply, response) = mpsc::channel();
        self.reads
            .send(Read {
                path: path.into(),
                thread: thread::current().id(),
                name: thread::current().name().map(str::to_owned),
                reply,
            })
            .unwrap();
        match response.recv_timeout(WAIT).unwrap() {
            Reply::Bytes(bytes) => bytes,
            Reply::Panic => panic!("synthetic loader panic"),
        }
    }
}

struct CallerLoader;

impl Loader for CallerLoader {
    fn read(&self, _: &str) -> Option<Vec<u8>> {
        panic!("background playback must not use the caller's loader")
    }
}

fn engine_with_music(music: MusicTable) -> (Engine, Receiver<Read>) {
    let (reads, received) = mpsc::channel();
    let sounds = sound_list::parse(b"1 sound/test.wav 3\n2 sound/other.wav 2\n").unwrap();
    let engine =
        Engine::silent(sounds, music, 100, 100).with_background_loader(ControlledLoader { reads });
    (engine, received)
}

fn engine() -> (Engine, Receiver<Read>) {
    engine_with_music(MusicTable::default())
}

fn read(received: &Receiver<Read>, expected: &str) -> Read {
    let read = received.recv_timeout(WAIT).unwrap();
    assert_eq!(read.path, expected);
    read
}

fn wave(sample: i16) -> Vec<u8> {
    let mut bytes = b"RIFF".to_vec();
    bytes.extend_from_slice(&40_u32.to_le_bytes());
    bytes.extend_from_slice(b"WAVEfmt ");
    bytes.extend_from_slice(&16_u32.to_le_bytes());
    bytes.extend_from_slice(&1_u16.to_le_bytes());
    bytes.extend_from_slice(&1_u16.to_le_bytes());
    bytes.extend_from_slice(&8000_u32.to_le_bytes());
    bytes.extend_from_slice(&16000_u32.to_le_bytes());
    bytes.extend_from_slice(&2_u16.to_le_bytes());
    bytes.extend_from_slice(&16_u16.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&4_u32.to_le_bytes());
    bytes.extend_from_slice(&sample.to_le_bytes());
    bytes.extend_from_slice(&sample.to_le_bytes());
    bytes
}

fn mp3() -> Vec<u8> {
    crate::music::behavior_tests::stereo_mp3()
}

fn finish(engine: &Engine) -> Vec<(u64, bool)> {
    let deadline = Instant::now() + WAIT;
    loop {
        let ready = {
            let loading = engine.loading.as_ref().unwrap().lock().unwrap();
            let queue = loading.worker.shared.queue.lock().unwrap();
            !queue.ready.is_empty()
        };
        if ready {
            return engine.take_playback_results();
        }
        assert!(
            Instant::now() < deadline,
            "loader did not publish its result"
        );
        thread::sleep(Duration::from_millis(1));
    }
}

#[test]
fn loading_reads_on_its_worker_without_holding_the_mixer() {
    let (engine, received) = engine();
    assert_eq!(
        engine.play_checked(1, &CallerLoader, false, 10),
        PlayResult::Pending
    );
    let request = read(&received, "sound/test.wav");
    assert_ne!(request.thread, thread::current().id());
    assert_eq!(request.name.as_deref(), Some("nwyd-audio-load"));
    assert!(engine.mixer.try_lock().is_ok());
    assert!(!engine.is_playing(1));
    assert!(engine.mixer().bank.loaded(1).is_none());
    request.respond(Some(wave(1234)));
    assert_eq!(finish(&engine), [(10, true)]);
    assert!(engine.is_playing(1));
    assert_eq!(
        engine.mixer().bank.loaded(1).unwrap().wave().samples,
        [1234; 2]
    );
    assert!(received.try_recv().is_err());
}

#[test]
fn loading_coalesces_reads_preserving_once_and_overlapping_request_order() {
    let (engine, received) = engine();
    assert_eq!(
        engine.play_checked(1, &CallerLoader, false, 1),
        PlayResult::Pending
    );
    let request = read(&received, "sound/test.wav");
    assert_eq!(
        engine.play_checked(1, &CallerLoader, false, 2),
        PlayResult::Ready(false)
    );
    assert_eq!(
        engine.play_checked(1, &CallerLoader, true, 3),
        PlayResult::Pending
    );
    assert_eq!(
        engine.play_checked(1, &CallerLoader, false, 4),
        PlayResult::Ready(false)
    );
    assert_eq!(
        engine.play_checked(1, &CallerLoader, true, 5),
        PlayResult::Pending
    );
    request.respond(Some(wave(1000)));
    assert_eq!(finish(&engine), [(1, true), (3, true), (5, true)]);
    assert_eq!(engine.mixer().bank.loaded(1).unwrap().active().count(), 3);
    assert!(received.try_recv().is_err());
}

#[test]
fn loading_preload_joins_later_play_and_loop_requests_preserve_polyphony() {
    let (engine, received) = engine();
    engine.preload(1, &CallerLoader);
    let request = read(&received, "sound/test.wav");
    assert!(!engine.is_playing(1));
    engine.play_looping(1, &CallerLoader);
    engine.play_looping(1, &CallerLoader);
    assert_eq!(
        engine.play_checked(1, &CallerLoader, false, 1),
        PlayResult::Ready(false)
    );
    assert_eq!(
        engine.play_checked(1, &CallerLoader, true, 2),
        PlayResult::Pending
    );
    request.respond(Some(wave(1000)));
    assert_eq!(finish(&engine), [(2, true)]);
    assert_eq!(
        engine
            .mixer()
            .bank
            .loaded(1)
            .unwrap()
            .active()
            .collect::<Vec<_>>(),
        [(0, true), (0, false)]
    );
    assert!(received.try_recv().is_err());
}

#[test]
fn loading_stop_cancels_pending_playback_but_keeps_the_loaded_sound() {
    let (engine, received) = engine();
    assert_eq!(
        engine.play_checked(1, &CallerLoader, false, 1),
        PlayResult::Pending
    );
    let request = read(&received, "sound/test.wav");
    engine.stop(1);
    assert!(!engine.is_playing(1));
    assert_eq!(engine.take_playback_results(), [(1, false)]);
    request.respond(Some(wave(1000)));
    assert!(finish(&engine).is_empty());
    assert!(!engine.is_playing(1));
    assert!(engine.mixer().bank.loaded(1).is_some());
    assert_eq!(
        engine.play_checked(1, &CallerLoader, false, 2),
        PlayResult::Ready(true)
    );
    assert!(received.try_recv().is_err());
}

#[test]
fn loading_release_and_reload_reject_the_stale_generation() {
    let (engine, received) = engine();
    assert_eq!(
        engine.play_checked(1, &CallerLoader, false, 1),
        PlayResult::Pending
    );
    let old = read(&received, "sound/test.wav");
    engine.release(1);
    assert_eq!(engine.take_playback_results(), [(1, false)]);
    assert_eq!(
        engine.play_checked(1, &CallerLoader, false, 2),
        PlayResult::Pending
    );
    old.respond(Some(wave(1000)));
    let current = read(&received, "sound/test.wav");
    assert!(engine.take_playback_results().is_empty());
    assert!(engine.mixer().bank.loaded(1).is_none());
    current.respond(Some(wave(2000)));
    assert_eq!(finish(&engine), [(2, true)]);
    assert_eq!(
        engine.mixer().bank.loaded(1).unwrap().wave().samples,
        [2000; 2]
    );
}

#[test]
fn loading_cancelled_receipts_do_not_play_or_report_and_allow_a_new_once_request() {
    let (engine, received) = engine();
    assert_eq!(
        engine.play_checked(1, &CallerLoader, false, 1),
        PlayResult::Pending
    );
    let request = read(&received, "sound/test.wav");
    engine.cancel_pending(1);
    assert!(!engine.is_playing(1));
    assert_eq!(
        engine.play_checked(1, &CallerLoader, false, 2),
        PlayResult::Pending
    );
    request.respond(Some(wave(1000)));
    assert_eq!(finish(&engine), [(2, true)]);
    assert_eq!(engine.mixer().bank.loaded(1).unwrap().active().count(), 1);
}

#[test]
fn loading_failures_are_cached_until_release_allows_retry() {
    for bytes in [None, Some(b"invalid wave".to_vec())] {
        let (engine, received) = engine();
        assert_eq!(
            engine.play_checked(1, &CallerLoader, false, 1),
            PlayResult::Pending
        );
        read(&received, "sound/test.wav").respond(bytes);
        assert_eq!(finish(&engine), [(1, false)]);
        assert_eq!(
            engine.play_checked(1, &CallerLoader, false, 2),
            PlayResult::Ready(false)
        );
        assert!(received.try_recv().is_err());
        engine.release(1);
        assert_eq!(
            engine.play_checked(1, &CallerLoader, false, 3),
            PlayResult::Pending
        );
        read(&received, "sound/test.wav").respond(Some(wave(1000)));
        assert_eq!(finish(&engine), [(3, true)]);
    }
}

#[test]
fn loading_muting_before_completion_prevents_pending_playback() {
    let (engine, received) = engine();
    assert_eq!(
        engine.play_checked(1, &CallerLoader, false, 1),
        PlayResult::Pending
    );
    let request = read(&received, "sound/test.wav");
    engine.mixer().bank.set_volume(volume::MIN);
    request.respond(Some(wave(1000)));
    assert_eq!(finish(&engine), [(1, false)]);
    assert!(!engine.is_playing(1));
    assert!(engine.mixer().bank.loaded(1).is_some());
}

#[test]
fn loading_pending_play_limit_refuses_excess_without_an_extra_read() {
    let (engine, received) = engine();
    assert_eq!(
        engine.play_checked(1, &CallerLoader, true, 0),
        PlayResult::Pending
    );
    let request = read(&received, "sound/test.wav");
    for receipt in 1..MAX_PENDING_PLAYS as u64 {
        assert_eq!(
            engine.play_checked(1, &CallerLoader, true, receipt),
            PlayResult::Pending
        );
    }
    assert_eq!(
        engine.play_checked(1, &CallerLoader, true, MAX_PENDING_PLAYS as u64),
        PlayResult::Ready(false)
    );
    request.respond(Some(wave(1000)));
    let results = finish(&engine);
    assert_eq!(results.len(), MAX_PENDING_PLAYS);
    assert!(results.iter().all(|(_, played)| *played));
    assert_eq!(engine.mixer().bank.loaded(1).unwrap().active().count(), 3);
    assert!(received.try_recv().is_err());
}

#[test]
fn loading_music_uses_a_present_override_without_reading_it_twice() {
    let table = music::parse(b"                 music/custom.mp3\n").unwrap();
    let (engine, received) = engine_with_music(table);
    assert!(engine.play_music(0, &CallerLoader));
    let request = read(&received, "music/custom.mp3");
    assert_ne!(request.thread, thread::current().id());
    assert!(engine.mixer.try_lock().is_ok());
    request.respond(Some(mp3()));
    assert!(finish(&engine).is_empty());
    assert_eq!(engine.mixer().music_slot(), Some(0));
    assert!(received.try_recv().is_err());
}

#[test]
fn loading_music_falls_back_only_when_the_override_is_missing() {
    let table = music::parse(b"                 music/custom.mp3\n").unwrap();
    let (engine, received) = engine_with_music(table);
    assert!(engine.play_music(0, &CallerLoader));
    read(&received, "music/custom.mp3").respond(None);
    read(&received, music::BUILTIN[0]).respond(Some(mp3()));
    assert!(finish(&engine).is_empty());
    assert_eq!(engine.mixer().music_slot(), Some(0));
    assert!(received.try_recv().is_err());
    assert!(engine.play_music(0, &CallerLoader));
    read(&received, "music/custom.mp3").respond(Some(b"invalid mp3".to_vec()));
    assert!(finish(&engine).is_empty());
    assert_eq!(engine.mixer().music_slot(), Some(0));
    assert!(received.try_recv().is_err());
}

#[test]
fn loading_music_stop_prevents_an_inflight_track_from_being_installed() {
    let (engine, received) = engine();
    assert!(engine.play_music(0, &CallerLoader));
    let request = read(&received, music::BUILTIN[0]);
    engine.stop_music();
    request.respond(Some(mp3()));
    assert!(finish(&engine).is_empty());
    assert_eq!(engine.mixer().music_slot(), None);
}

#[test]
fn loading_music_switch_coalesces_queued_tracks_and_rejects_the_inflight_track() {
    let (engine, received) = engine();
    assert!(engine.play_music(0, &CallerLoader));
    let old = read(&received, music::BUILTIN[0]);
    assert!(engine.play_music(1, &CallerLoader));
    assert!(engine.play_music(2, &CallerLoader));
    old.respond(Some(mp3()));
    let current = read(&received, music::BUILTIN[2]);
    assert!(engine.take_playback_results().is_empty());
    assert_eq!(engine.mixer().music_slot(), None);
    current.respond(Some(mp3()));
    assert!(finish(&engine).is_empty());
    assert_eq!(engine.mixer().music_slot(), Some(2));
    assert!(received.try_recv().is_err());
}

#[test]
fn loading_loader_panics_report_failure_and_leave_the_worker_usable() {
    let (engine, received) = engine();
    assert_eq!(
        engine.play_checked(1, &CallerLoader, false, 1),
        PlayResult::Pending
    );
    read(&received, "sound/test.wav")
        .reply
        .send(Reply::Panic)
        .unwrap();
    assert_eq!(finish(&engine), [(1, false)]);
    assert!(engine.play_music(0, &CallerLoader));
    read(&received, music::BUILTIN[0])
        .reply
        .send(Reply::Panic)
        .unwrap();
    assert!(finish(&engine).is_empty());
    assert_eq!(engine.mixer().music_slot(), None);
    assert_eq!(
        engine.play_checked(2, &CallerLoader, false, 2),
        PlayResult::Pending
    );
    read(&received, "sound/other.wav").respond(Some(wave(2000)));
    assert_eq!(finish(&engine), [(2, true)]);
}

#[test]
fn loading_dropping_engine_does_not_wait_for_a_blocked_loader_or_run_queued_jobs() {
    let (engine, received) = engine();
    assert_eq!(
        engine.play_checked(1, &CallerLoader, false, 1),
        PlayResult::Pending
    );
    let request = read(&received, "sound/test.wav");
    assert_eq!(
        engine.play_checked(2, &CallerLoader, false, 2),
        PlayResult::Pending
    );
    let (dropped, completion) = mpsc::channel();
    let dropper = thread::spawn(move || {
        drop(engine);
        dropped.send(()).unwrap();
    });
    let result = completion.recv_timeout(WAIT);
    request.respond(Some(wave(1000)));
    result.unwrap();
    dropper.join().unwrap();
    assert!(matches!(
        received.recv_timeout(WAIT),
        Err(mpsc::RecvTimeoutError::Disconnected)
    ));
}

#[test]
fn loading_cancelled_results_count_toward_the_pending_limit_until_consumed() {
    let (engine, received) = engine();
    engine.preload(1, &CallerLoader);
    let request = read(&received, "sound/test.wav");
    for receipt in 0..MAX_PENDING_PLAYS as u64 {
        assert_eq!(
            engine.play_checked(1, &CallerLoader, true, receipt),
            PlayResult::Pending
        );
        engine.stop(1);
    }
    assert_eq!(
        engine.play_checked(1, &CallerLoader, true, MAX_PENDING_PLAYS as u64),
        PlayResult::Ready(false)
    );
    let results = engine.take_playback_results();
    assert_eq!(results.len(), MAX_PENDING_PLAYS);
    assert!(results.iter().all(|(_, played)| !played));
    assert_eq!(
        engine.play_checked(1, &CallerLoader, true, MAX_PENDING_PLAYS as u64),
        PlayResult::Pending
    );
    request.respond(Some(wave(1000)));
    assert_eq!(finish(&engine), [(MAX_PENDING_PLAYS as u64, true)]);
}
