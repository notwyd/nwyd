use super::{Error, Track};
use crate::{Bank, Mixer, volume};
use formats::audio::SoundList;

const MP3_RATE: u32 = 8000;
const PACKET_BYTES: usize = 216;
const PACKET_FRAMES: usize = 576;
const STEREO_PACKETS: usize = 4;

const STEREO_TONE_MP3: &str = concat!(
    "ffe33864000e546b502fa788000b9892c97943000206936e50007efdfbf7f1ef7bd29478f225294b",
    "ddfbf7f1f7eef2a9f2de2de26e2e65cd575bfa529018158879a6699a675a8dfef041df2fe18e5dfc",
    "e74f2e081c8801307f07c103928030fe4c10765cff774ffcb83e0f87c100401071707c1fddf2e0f8",
    "7c1004010040060f83e0fc60200870fd5043fe03bbbbbbff111113f77777ffdc0c0c0c0c5bc40182",
    "e0e020081cfffffe8f7f28ed00ff021cd00ff021ce1f0800000928820ab4aa1a99ffffde7fd8cbb3",
    "1276bffff0fbd4ea64e89c2180615f60ffe33844220cd8d158c8cd20001a299ea0c987a000e1886f",
    "2dcbcdc31563074054054a1616fff00e41863d922a2accd2bfffe2391461767a2c334aac337ffffd",
    "18f67a180f894240d15ff02183e0441ffffc0883e0445fffffa01001a40f0d8461dff1a88a3c36f5",
    "73d0170a41e802fb1879e43866801400002f018c4191a4a910030c981156258ae4c092008c018a89",
    "021a39a404c8bc5e312eff904333737418d132ea4924b45152497fee6e831a07c1c1a0a888f02bfd",
    "01f1c868efffd0d1c8bd0a264b06e40bfc9b25859a399fe2c401a580d7b001a6ffe33844150bcd35",
    "5400ca400017a30a9801879001dcc7c2cf0b8c64459a4408922caff90227caa434d0ac4d1bffff9a",
    "932686c5e37352ea07fffffcc933c6281f324cf18a0ffffffe649b18a0e649b18910afffe0122150",
    "088c2a0888c2b755ff3478a12ff114521905dfe33c29606f5001e0a185cee628b25f87a231066872",
    "89320a434a9fffe4f13457274993436268dcd7ffffc9934362f266a6281f324cf7fffff98a07cc93",
    "3c62839926c6287ffffffb992d8c54e651090744254c414d45332e31303055555555555555555555",
    "5555555555555555ffe338641b000001a400e000000000034801c000005555555555555555555555",
    "55555555555555555555555555555555555555555555555555555555555555555555555555555555",
    "55555555555555555555555555555555555555555555555555555555555555555555555555555555",
    "55555555555555555555555555555555555555555555555555555555555555555555555555555555",
    "55555555555555555555555555555555555555555555555555555555555555555555555555555555",
    "555555555555555555555555555555555555555555555555",
);

const MONO_TONE_MP3: &str = concat!(
    "ffe318c4000c308ebdb8089260a8009db771c63663c6e405003e6f3bc82130b8264fc4921dfa3ea5",
    "94413b6ed5f77fb9545bdd94939fb8a516cfffffdfa1feb01cbb7e00d9999998ffe318c40a0ce89e",
    "da581846603292a96abc6f66c584044c023d44915709622875674ab8ac8cf2ce95715919e5b9712c",
    "8cf2dc568b2f5a23f964ffb2c9594080450188023ccb634effe318c4110b58be000008cc08122850",
    "188264138b628b11998a0b371566a16e2acd42dc57a9bc57a9b54c414d45332e3130305555555555",
    "55555555555555555555555555555555",
);

fn mp3(hex: &str) -> Vec<u8> {
    hex.as_bytes()
        .chunks_exact(2)
        .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
        .collect()
}

fn stereo_track() -> Track {
    Track::open(stereo_mp3(), "synthetic-stereo.mp3").unwrap()
}

pub(crate) fn stereo_mp3() -> Vec<u8> {
    mp3(STEREO_TONE_MP3)
}

fn read_frames(track: &mut Track, count: usize) -> Vec<[f32; 2]> {
    (0..count).map(|_| track.next_frame().unwrap()).collect()
}

fn music_mixer(device_rate: u32) -> Mixer {
    let bank = Bank::new(SoundList::default(), volume::MAX);

    let mut mixer = Mixer::new(bank, volume::MAX, device_rate);

    mixer.play_music(7, stereo_track());

    mixer
}

fn render(mixer: &mut Mixer, frames: usize) -> Vec<f32> {
    let mut output = vec![f32::NAN; frames * 2];

    mixer.fill(&mut output);

    output
}

#[test]
fn music_decodes_mono_into_matching_nonzero_stereo_channels() {
    let mut track = Track::open(mp3(MONO_TONE_MP3), "synthetic-mono.mp3").unwrap();

    assert_eq!(track.rate(), MP3_RATE);

    assert!(!track.ended());

    let frames = read_frames(&mut track, 3 * PACKET_FRAMES);

    assert!(frames.iter().all(|[left, right]| left == right));

    assert!(frames.iter().flatten().all(|sample| sample.is_finite()));

    assert!(frames.iter().any(|[left, _]| left.abs() > 0.01));
}

#[test]
fn music_decodes_distinct_stereo_channels_at_the_source_rate() {
    let mut track = stereo_track();

    assert_eq!(track.rate(), MP3_RATE);

    let frames = read_frames(&mut track, STEREO_PACKETS * PACKET_FRAMES);

    assert!(frames.iter().flatten().all(|sample| sample.is_finite()));

    assert!(
        frames
            .iter()
            .any(|[left, right]| (left - right).abs() > 0.01)
    );

    assert!(frames.iter().any(|[left, _]| left.abs() > 0.1));

    assert!(frames.iter().any(|[_, right]| right.abs() > 0.05));

    assert!(!track.ended());
}

#[test]
fn music_rewinds_at_eof_and_resets_decoder_history_for_each_loop() {
    let mut track = stereo_track();

    let first = read_frames(&mut track, STEREO_PACKETS * PACKET_FRAMES);

    for _ in 0..2 {
        assert_eq!(read_frames(&mut track, first.len()), first);

        assert!(!track.ended());
    }
}

#[test]
fn music_rejects_empty_invalid_and_truncated_headers_with_asset_name() {
    for bytes in [vec![], b"invalid mp3".to_vec(), vec![0xff, 0xe3, 0x38]] {
        assert_eq!(
            Track::open(bytes, "broken.mp3").unwrap_err(),
            Error::Undecodable("broken.mp3".to_owned())
        );
    }
}

#[test]
fn music_loops_the_complete_packets_before_a_truncated_final_packet() {
    let mut bytes = mp3(STEREO_TONE_MP3);

    bytes.pop();

    let mut track = Track::open(bytes, "truncated.mp3").unwrap();

    let expected = read_frames(&mut stereo_track(), 3 * PACKET_FRAMES);

    for _ in 0..2 {
        assert_eq!(read_frames(&mut track, expected.len()), expected);

        assert!(!track.ended());
    }
}

#[test]
fn music_skips_a_corrupt_packet_and_resumes_decoding_after_rewind() {
    let mut bytes = mp3(STEREO_TONE_MP3);

    bytes[3 * PACKET_BYTES + 4..3 * PACKET_BYTES + 21].fill(0xff);

    let mut track = Track::open(bytes, "corrupt-packet.mp3").unwrap();

    let expected = read_frames(&mut stereo_track(), 3 * PACKET_FRAMES);

    for _ in 0..2 {
        assert_eq!(read_frames(&mut track, expected.len()), expected);

        assert!(!track.ended());
    }
}

#[test]
fn music_stops_after_one_complete_pass_without_decodable_audio() {
    let mut bytes = mp3(STEREO_TONE_MP3);

    for packet in bytes.chunks_exact_mut(PACKET_BYTES) {
        packet[4..21].fill(0xff);
    }

    let mut track = Track::open(bytes, "all-corrupt.mp3").unwrap();

    for _ in 0..=STEREO_PACKETS {
        track.fill();
    }

    assert!(track.ended());

    assert_eq!(track.next_frame(), None);

    assert_eq!(track.next_frame(), None);
}

#[test]
fn music_recovers_after_a_long_corrupt_prefix() {
    let valid = mp3(STEREO_TONE_MP3);

    let mut corrupt_packet = valid[..PACKET_BYTES].to_vec();

    corrupt_packet[4..21].fill(0xff);

    let mut bytes = corrupt_packet.repeat(600);

    bytes.extend_from_slice(&valid);

    let mut track = Track::open(bytes, "corrupt-prefix.mp3").unwrap();

    let expected = read_frames(&mut stereo_track(), STEREO_PACKETS * PACKET_FRAMES);

    for _ in 0..2 {
        assert_eq!(read_frames(&mut track, expected.len()), expected);

        assert!(!track.ended());
    }
}

#[test]
fn music_mixing_is_equivalent_across_callback_sizes_and_sample_rates() {
    for device_rate in [4000, MP3_RATE, 11025] {
        let mut complete = music_mixer(device_rate);

        let mut chunked = music_mixer(device_rate);

        let chunks = [1, 3, 571, 2, 2305, 7, 2111];

        let expected = render(&mut complete, chunks.iter().sum());

        let actual: Vec<_> = chunks
            .into_iter()
            .flat_map(|frames| render(&mut chunked, frames))
            .collect();

        assert_eq!(actual, expected, "device rate {device_rate}");

        assert!(actual.iter().any(|sample| sample.abs() > 0.01));

        assert_eq!(complete.music_slot(), Some(7));

        assert_eq!(chunked.music_slot(), Some(7));
    }
}

#[test]
fn inactive_music_mixer_emits_silence_without_advancing_playback() {
    let mut reference = music_mixer(11025);

    let mut paused = music_mixer(11025);

    assert_eq!(render(&mut paused, 901), render(&mut reference, 901));

    paused.set_active(false);

    assert_eq!(render(&mut paused, 137), vec![0.0; 274]);

    assert_eq!(paused.music_slot(), Some(7));

    paused.set_active(true);

    assert_eq!(render(&mut paused, 1800), render(&mut reference, 1800));
}

#[test]
fn muted_music_keeps_advancing_and_volume_changes_apply_to_current_position() {
    let mut reference = music_mixer(11025);

    let mut muted = music_mixer(11025);

    assert_eq!(render(&mut muted, 901), render(&mut reference, 901));

    muted.set_music_volume(volume::MIN);

    assert_eq!(render(&mut muted, 200), vec![0.0; 400]);

    render(&mut reference, 200);

    muted.set_music_volume(-2000);

    let actual = render(&mut muted, 1000);

    let expected = render(&mut reference, 1000);

    for (actual, expected) in actual.into_iter().zip(expected) {
        assert!((actual - expected * 0.1).abs() < 0.000001);
    }
}

#[test]
fn stopping_music_clears_its_slot_and_restarting_resets_resampling() {
    let mut mixer = music_mixer(11025);

    render(&mut mixer, 901);

    mixer.stop_music();

    assert_eq!(mixer.music_slot(), None);

    assert_eq!(render(&mut mixer, 317), vec![0.0; 634]);

    mixer.play_music(9, stereo_track());

    assert_eq!(mixer.music_slot(), Some(9));

    assert_eq!(
        render(&mut mixer, 2000),
        render(&mut music_mixer(11025), 2000)
    );
}

#[test]
fn replacing_playing_music_resets_resampling_and_updates_its_slot() {
    let mut mixer = music_mixer(11025);

    render(&mut mixer, 901);

    mixer.play_music(11, stereo_track());

    assert_eq!(mixer.music_slot(), Some(11));

    assert_eq!(
        render(&mut mixer, 2000),
        render(&mut music_mixer(11025), 2000)
    );
}

#[cfg(not(target_arch = "wasm32"))]
mod prebuffer {
    use super::*;

    #[test]
    fn music_prebuffer_preserves_partial_packets_and_loop_output() {
        let mut buffered = stereo_track();
        let mut reference = stereo_track();

        assert_eq!(
            read_frames(&mut buffered, 17),
            read_frames(&mut reference, 17)
        );

        for count in [0, 1, 64, 512, 577, 2305, 8, 9000] {
            assert!(buffered.buffer_frames(count, || false));
            assert!(buffered.buffer.len() - buffered.cursor >= count);
            assert_eq!(
                read_frames(&mut buffered, count),
                read_frames(&mut reference, count)
            );
        }
    }

    #[test]
    fn music_prebuffer_does_not_decode_or_compact_when_enough_pcm_is_available() {
        let mut track = stereo_track();
        assert!(track.buffer_frames(1000, || false));
        read_frames(&mut track, 17);

        let cursor = track.cursor;
        let length = track.buffer.len();
        let allocation = track.buffer.as_ptr();
        let mut checks = 0;

        assert!(track.buffer_frames(1000, || {
            checks += 1;
            false
        }));

        assert_eq!(checks, 1);
        assert_eq!(track.cursor, cursor);
        assert_eq!(track.buffer.len(), length);
        assert_eq!(track.buffer.as_ptr(), allocation);
    }

    #[test]
    fn music_prebuffer_cancellation_keeps_every_frame_decoded_so_far() {
        let mut buffered = stereo_track();
        let mut reference = stereo_track();

        assert_eq!(
            read_frames(&mut buffered, 17),
            read_frames(&mut reference, 17)
        );

        assert!(!buffered.buffer_frames(9000, || true));
        assert_eq!(
            read_frames(&mut buffered, 23),
            read_frames(&mut reference, 23)
        );

        let mut checks = 0;
        assert!(!buffered.buffer_frames(9000, || {
            checks += 1;
            checks == 4
        }));

        assert_eq!(checks, 4);
        assert!(!buffered.ended());
        assert!(buffered.buffer.len() - buffered.cursor > PACKET_FRAMES);
        assert_eq!(
            read_frames(&mut buffered, 29),
            read_frames(&mut reference, 29)
        );

        assert!(buffered.buffer_frames(9000, || false));
        assert_eq!(
            read_frames(&mut buffered, 9000),
            read_frames(&mut reference, 9000)
        );
    }

    #[test]
    fn music_prebuffer_checks_cancellation_between_corrupt_packets_and_can_resume() {
        let valid = stereo_mp3();
        let mut corrupt = valid[..PACKET_BYTES].to_vec();
        corrupt[4..21].fill(0xff);
        let mut bytes = corrupt.repeat(600);
        bytes.extend_from_slice(&valid);
        let mut track = Track::open(bytes, "cancel-corrupt-prefix.mp3").unwrap();
        let mut checks = 0;

        assert!(!track.buffer_frames(1, || {
            checks += 1;
            checks == 7
        }));

        assert_eq!(checks, 7);
        assert!(!track.ended());
        assert!(track.buffer.is_empty());
        assert!(track.buffer_frames(STEREO_PACKETS * PACKET_FRAMES, || false));
        assert_eq!(
            read_frames(&mut track, STEREO_PACKETS * PACKET_FRAMES),
            read_frames(&mut stereo_track(), STEREO_PACKETS * PACKET_FRAMES)
        );
    }

    #[test]
    fn music_prebuffer_preserves_truncated_and_corrupt_loop_recovery() {
        let mut truncated = stereo_mp3();
        truncated.pop();
        let mut corrupt = stereo_mp3();
        corrupt[3 * PACKET_BYTES + 4..3 * PACKET_BYTES + 21].fill(0xff);

        for bytes in [truncated, corrupt] {
            let mut buffered = Track::open(bytes.clone(), "buffered.mp3").unwrap();
            let mut reference = Track::open(bytes, "direct.mp3").unwrap();

            for count in [1, 91, 4096, 57, 2048] {
                assert!(buffered.buffer_frames(count, || false));
                assert_eq!(
                    read_frames(&mut buffered, count),
                    read_frames(&mut reference, count)
                );
            }
        }
    }

    #[test]
    fn music_prebuffer_finishes_a_wholly_corrupt_stream_without_filling_the_request() {
        let mut bytes = stereo_mp3();
        for packet in bytes.chunks_exact_mut(PACKET_BYTES) {
            packet[4..21].fill(0xff);
        }
        let mut track = Track::open(bytes, "corrupt.mp3").unwrap();
        let mut checks = 0;

        assert!(track.buffer_frames(1024, || {
            checks += 1;
            assert!(checks <= STEREO_PACKETS + 2);
            false
        }));

        assert!(track.ended());
        assert_eq!(track.next_frame(), None);
        assert!(track.buffer_frames(1024, || false));
    }
}
