use super::*;
use config::config_bin::{self, Config, Language, slot};
use data::{envelope, quick_slot};
use error::Kind;
use text::{Codepage, codepage};

#[test]
fn binary_reads_decode_unaligned_little_endian_values() {
    let bytes = [0, 0x78, 0x56, 0x34, 0x12];

    assert_eq!(util::u32_at(&bytes, 1, Kind::Config).unwrap(), 0x12345678);

    assert_eq!(util::u16_at(&bytes, 2, Kind::Config).unwrap(), 0x3456);

    assert_eq!(util::byte_at(&bytes, 4, Kind::Config).unwrap(), 0x12);
}

#[test]
fn binary_reads_reject_truncated_and_overflowing_offsets() {
    for offset in [1, usize::MAX - 1, usize::MAX] {
        assert!(matches!(
            util::u32_at(&[0; 4], offset, Kind::Config),
            Err(Error::Truncated { .. })
        ));
    }

    for offset in [3, usize::MAX] {
        assert!(matches!(
            util::u16_at(&[0; 4], offset, Kind::Config),
            Err(Error::Truncated { .. })
        ));
    }

    assert!(matches!(
        util::byte_at(&[], 0, Kind::Config),
        Err(Error::Truncated { needed: 1, .. })
    ));
}

#[test]
fn config_preserves_signed_words_in_little_endian_order() {
    let mut config = Config {
        raw: std::array::from_fn(|i| i as i16 - 8),
    };

    config.raw[0] = i16::MIN;

    config.raw[1] = i16::MAX;

    let bytes = config.to_bytes();

    assert_eq!(&bytes[..6], &[0, 0x80, 0xff, 0x7f, 0xfa, 0xff]);

    assert_eq!(config_bin::parse(&bytes).unwrap(), config);
}

#[test]
fn config_rejects_all_short_files_and_ignores_trailing_bytes() {
    let config = Config::default();

    let mut bytes = config.to_bytes().to_vec();

    for length in 0..config_bin::LEN {
        assert!(matches!(
            config_bin::parse(&bytes[..length]),
            Err(Error::Truncated {
                kind: Kind::Config,
                ..
            })
        ));
    }

    bytes.extend_from_slice(b"synthetic trailing data");

    assert_eq!(config_bin::parse(&bytes).unwrap(), config);
}

#[test]
fn config_resolution_is_one_based_with_fallback_for_invalid_indices() {
    let mut config = Config::default();

    for (index, preset) in config_bin::PRESETS.iter().enumerate() {
        config.raw[slot::RESOLUTION] = index as i16 + 1;

        assert_eq!(config.preset(), *preset);
    }

    for index in [i16::MIN, 0, 11, i16::MAX] {
        config.raw[slot::RESOLUTION] = index;

        assert_eq!(config.preset(), config_bin::FALLBACK);
    }
}

#[test]
fn config_distinguishes_unspecified_window_mode_and_clamps_brightness() {
    let mut config = Config::default();

    for (raw, expected) in [
        (-1, None),
        (0, Some(true)),
        (1, Some(false)),
        (2, Some(false)),
    ] {
        config.raw[slot::WINDOW_MODE] = raw;

        assert_eq!(config.fullscreen(), expected);
    }

    for (raw, expected) in [(-1, 0), (0, 0), (57, 57), (100, 100), (101, 100)] {
        config.raw[slot::BRIGHTNESS] = raw;

        assert_eq!(config.brightness(), expected);
    }

    assert_eq!(Language::from_code(3), Language::Portuguese);

    assert_eq!(Language::from_code(-1), Language::English);

    assert_eq!(Language::from_code(99), Language::English);
}

#[test]
fn windows_codepage_handles_typographic_characters_and_lossy_replacements() {
    let encoded = b"\x80 \x93Ol\xe1\x94 \x97 \x99";

    let text = "€ “Olá” — ™";

    assert_eq!(codepage::decode(encoded, Codepage::Windows1252), text);

    assert_eq!(codepage::encode(text, Codepage::Windows1252), encoded);

    assert_eq!(codepage::encode("A界🦀", Codepage::Windows1252), b"A??");
}

#[test]
fn windows_codepage_round_trips_every_byte() {
    let bytes: Vec<u8> = (0..=255).collect();

    let decoded = codepage::decode(&bytes, Codepage::Windows1252);

    assert_eq!(codepage::encode(&decoded, Codepage::Windows1252), bytes);
}

#[test]
fn envelope_decodes_xor_and_requires_a_valid_checksum_trailer() {
    let encoded = [0x5a, 0xff, 0];

    assert_eq!(
        envelope::open(&encoded, 3, envelope::Integrity::None, Kind::QuickSlot).unwrap(),
        [0, 0xa5, 0x5a]
    );

    let mut bytes = encoded.to_vec();

    let checksum = envelope::seven_step_checksum(&bytes);

    bytes.extend_from_slice(&checksum.to_le_bytes());

    assert_eq!(
        envelope::open(&bytes, 3, envelope::Integrity::TrailerB, Kind::QuickSlot).unwrap(),
        [0, 0xa5, 0x5a]
    );

    bytes[3] ^= 1;

    assert!(matches!(
        envelope::open(&bytes, 3, envelope::Integrity::TrailerB, Kind::QuickSlot),
        Err(Error::Structure { .. })
    ));

    assert!(matches!(
        envelope::open(&encoded, 3, envelope::Integrity::TrailerB, Kind::QuickSlot),
        Err(Error::Truncated {
            offset: 3,
            needed: 4,
            ..
        })
    ));
}

#[test]
fn synthetic_quick_slots_round_trip_and_pad_unused_records() {
    let slot = quick_slot::Slot {
        char_name: "test-character".to_owned(),
        slot: [0, 1, 255, 65535, u32::MAX],
        last_used: 20,
    };

    let bytes = quick_slot::to_bytes(std::slice::from_ref(&slot));

    assert_eq!(bytes.len(), quick_slot::LEN);

    let decoded = quick_slot::parse(&bytes, Codepage::Windows1252).unwrap();

    assert_eq!(decoded.len(), quick_slot::RECORDS);

    assert_eq!(decoded[0], slot);

    assert!(
        decoded[1..]
            .iter()
            .all(|entry| entry == &quick_slot::Slot::default())
    );

    assert!(quick_slot::parse(&bytes[..quick_slot::BODY_LEN], Codepage::Windows1252).is_err());
}

#[test]
fn quick_slot_claim_keeps_matching_bindings_but_clears_evicted_bindings() {
    let mut slots: Vec<_> = (0..3)
        .map(|index| quick_slot::Slot {
            char_name: format!("test-{index}"),
            slot: [index + 1; quick_slot::SLOTS],
            last_used: index + 1,
        })
        .collect();

    assert_eq!(quick_slot::claim(&mut slots, "test-1", 10), Some(1));

    assert_eq!(slots[1].slot, [2; 5]);

    assert_eq!(slots[1].last_used, 10);

    assert_eq!(quick_slot::claim(&mut slots, "test-new", 11), Some(0));

    assert_eq!(slots[0].slot, [0; 5]);

    assert_eq!(slots[0].char_name, "test-new");

    assert_eq!(slots[0].last_used, 11);
}

#[test]
fn quick_slot_selection_prefers_free_records_and_does_not_evict_future_records() {
    let occupied = quick_slot::Slot {
        char_name: "test-occupied".to_owned(),
        last_used: 100,
        ..Default::default()
    };

    assert_eq!(
        quick_slot::select(
            &[occupied.clone(), quick_slot::Slot::default()],
            "test-new",
            200
        ),
        Some(1)
    );

    assert_eq!(quick_slot::select(&[occupied], "test-new", 100), None);

    assert_eq!(quick_slot::select(&[], "test-new", 100), None);
}

#[test]
fn sound_list_normalizes_paths_and_last_duplicate_wins() {
    let list =
        audio::sound_list::parse(b"1 sound\\old.wav 1\n2 sound/two.wav 3\n1 sound\\\\new.wav 2")
            .unwrap();

    assert_eq!(list.len(), 2);

    let entry = list.get_index(1).unwrap();

    assert_eq!(entry.path(), "sound/new.wav");

    assert_eq!(entry.voices, 2);

    assert_eq!(
        list.duplicates()
            .iter()
            .map(|slot| slot.get())
            .collect::<Vec<_>>(),
        [1]
    );

    assert_eq!(
        list.missing(|path| path == "sound/new.wav")
            .iter()
            .map(|slot| slot.get())
            .collect::<Vec<_>>(),
        [2]
    );
}

#[test]
fn sound_list_rejects_invalid_indices_and_malformed_numbers() {
    for bytes in [
        b"0 file.wav 1".as_slice(),
        b"512 file.wav 1",
        b"-1 file.wav 1",
        b"text file.wav 1",
        b"1 file.wav text",
    ] {
        assert!(audio::sound_list::parse(bytes).is_err());
    }

    let list = audio::sound_list::parse(b"1 sound/test.wav 1\n2 unfinished.wav").unwrap();

    assert_eq!(list.len(), 1);

    assert!(list.get_index(2).is_none());
}

#[test]
fn weather_preserves_unknown_codes_without_enabling_precipitation() {
    use net::weather::{self, Forecast};

    for (code, effect, stop) in [
        (0, false, false),
        (1, true, true),
        (2, true, true),
        (3, true, true),
        (u32::MAX, false, true),
    ] {
        let forecast = weather::parse(&code.to_le_bytes()).unwrap();

        assert_eq!(forecast, Forecast { code });

        assert_eq!(forecast.has_precipitation_effect(), effect);

        assert_eq!(forecast.stops_the_cycle(), stop);

        assert_eq!(forecast.to_bytes(), code.to_le_bytes());
    }

    for length in 0..weather::LEN {
        assert!(weather::parse(&vec![0; length]).is_err());
    }

    assert_eq!(
        weather::parse(&[3, 0, 0, 0, 0xff]).unwrap(),
        Forecast { code: 3 }
    );
}
