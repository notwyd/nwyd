use super::*;
use crate::scene::login_panels::language_tests::translated;

fn panels(word: &str) -> Panels {
    let mut panels = Panels::for_test();
    let strings = translated(&[(SHOUT_WORD_RECORD, word)], "");
    panels.strings = strings.clone();
    bind(&mut panels, &strings);
    panels
}

fn expected_packet(body: &[u8]) -> Vec<u8> {
    let mut packet = vec![0; 160];
    packet[4..6].copy_from_slice(&0x334u16.to_le_bytes());
    packet[6..8].copy_from_slice(&42u16.to_le_bytes());
    packet[12..16].copy_from_slice(b"spk\0");
    packet[28..28 + body.len()].copy_from_slice(body);
    packet
}

#[test]
fn localized_shouts_send_the_same_packet_and_preserve_echo_and_history() {
    for word in ["spk", "gritar"] {
        let mut panels = panels(word);
        let draft = format!("/{word} Olá Synthetic");
        let Enter::Sent(sent) = route_with_move_alpha(
            &mut panels,
            42,
            "Synthetic",
            1,
            0.5,
            &Reserved::default(),
            10000,
            &draft,
        ) else {
            panic!("localized shout /{word} must send immediately");
        };

        assert_eq!(sent.mode, SHOUT);
        assert_eq!(sent.text, "Olá Synthetic");
        assert_eq!(sent.packet, expected_packet(b"Ol\xe1 Synthetic"));
        assert_eq!(panels.chat.outbox, [sent.packet]);
        assert!(!sent.balloon);
        assert!(take_balloon(&mut panels).is_none());
        assert_eq!(panels.chat.whisper, word);
        assert_eq!(panels.chat.history[0], draft);
        assert_eq!(panels.chat.shouts[0], word);
        assert_eq!(panels.chat.sent_at, [10000, 0, 0, 0]);
        assert_eq!(panels.chat.kingdom_at, 0);
        assert_eq!(panels.chat.records.len(), 1);
        assert_eq!(
            panels.chat.records[0],
            Line {
                text: format!("[Synthetic] : {word}> Olá Synthetic"),
                color: 0xFFFF_FF00,
                speaker: None,
            }
        );
    }
}

#[test]
fn shout_bodies_keep_spacing_and_use_the_existing_text_filter() {
    let reserved = Reserved {
        rows: vec![(b"Synthetic".to_vec(), b"Filtered".to_vec())],
    };

    for (draft, body) in [
        ("/gritar", ""),
        ("/gritar ", ""),
        ("/gritar  Synthetic message", " Filtered message"),
    ] {
        let mut panels = panels("gritar");
        let Enter::Sent(sent) = route(&mut panels, 42, "Sender", 1, &reserved, 10000, draft) else {
            panic!("shout {draft:?} must send");
        };
        assert_eq!(sent.mode, SHOUT);
        assert_eq!(sent.text, body);
        assert_eq!(sent.packet, expected_packet(body.as_bytes()));
    }
}

#[test]
fn shouts_obey_existing_length_and_flood_limits() {
    let mut panels = panels("gritar");
    let reserved = Reserved::default();
    let too_long = format!("/gritar {}", "x".repeat(MAX_LINE - 8));
    assert_eq!(too_long.len(), MAX_LINE);
    assert_eq!(
        route(&mut panels, 42, "Sender", 1, &reserved, 10000, &too_long),
        Enter::Nothing
    );
    assert!(panels.chat.outbox.is_empty());
    assert_eq!(panels.chat.sent_at, [0; 4]);

    let longest = &too_long[..MAX_LINE - 1];
    for now in [10000, 10001, 10002, 10003] {
        assert!(matches!(
            route(&mut panels, 42, "Sender", 1, &reserved, now, longest),
            Enter::Sent(Sent { mode: SHOUT, .. })
        ));
    }
    assert_eq!(panels.chat.outbox.len(), 4);
    assert_eq!(
        route(&mut panels, 42, "Sender", 1, &reserved, 10004, longest),
        Enter::Nothing
    );
    assert_eq!(panels.chat.outbox.len(), 4);
    assert_eq!(panels.chat.records.len(), 4);
    assert!(matches!(
        route(&mut panels, 42, "Sender", 1, &reserved, 15000, longest),
        Enter::Sent(Sent { mode: SHOUT, .. })
    ));
}

#[test]
fn other_player_names_and_case_variants_keep_whisper_behavior() {
    for (language_word, target) in [("spk", "gritar"), ("gritar", "Gritar"), ("spk", "Spk")] {
        let mut panels = panels(language_word);
        let Enter::Sent(sent) = route(
            &mut panels,
            42,
            "Sender",
            1,
            &Reserved::default(),
            10000,
            &format!("/{target} Synthetic message"),
        ) else {
            panic!("non-command /{target} must remain a whisper");
        };
        assert_eq!(sent.mode, WHISPER);
        assert_eq!(&sent.packet[12..12 + target.len()], target.as_bytes());
        assert_eq!(sent.packet[12 + target.len()], 0);
    }
}
