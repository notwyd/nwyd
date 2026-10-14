use super::*;

fn frame(opcode: u16, payload: &[u8]) -> Vec<u8> {
    let mut bytes = vec![0; header::LEN];

    bytes.extend_from_slice(payload);

    header::set_opcode(&mut bytes, opcode);

    codec::seal(&mut bytes, 0x42, 0x1234, 0x01020304);

    bytes
}

#[test]
fn header_has_an_explicit_little_endian_wire_layout() {
    let header = Header {
        size: 0x0010,
        key: 0x23,
        checksum: 0x45,
        opcode: 0x6789,
        client_id: 0xabcd,
        tick: 0x01234567,
    };

    let expected = [
        0x10, 0, 0x23, 0x45, 0x89, 0x67, 0xcd, 0xab, 0x67, 0x45, 0x23, 1,
    ];

    assert_eq!(header.to_bytes(), expected);

    assert_eq!(Header::parse(&expected), Some(header));

    let mut extended = expected.to_vec();

    extended.extend_from_slice(b"test");

    assert_eq!(Header::parse(&extended), Some(header));
}

#[test]
fn header_rejects_every_truncated_prefix() {
    for length in 0..header::LEN {
        assert!(Header::parse(&vec![0; length]).is_none());
    }

    assert_eq!(header::opcode_of(&[0; 5]), None);

    assert_eq!(header::opcode_of(&[0, 0, 0, 0, 0x34, 0x12]), Some(0x1234));
}

#[test]
fn legal_sizes_include_both_limits() {
    for size in [0, 11, 10241, u16::MAX] {
        assert!(!Header::size_is_legal(size));
    }

    for size in [12, 13, 10240] {
        assert!(Header::size_is_legal(size));
    }
}

#[test]
fn cipher_matches_a_synthetic_zero_header_vector() {
    let mut plain = [12, 0, 0, 16, 0, 0, 0, 0, 0, 0, 0, 0];

    assert_eq!(cipher::checksum(0, 12), 16);

    cipher::encrypt(&mut plain, 0);

    assert_eq!(
        plain,
        [12, 0, 0, 16, 250, 241, 132, 249, 202, 240, 244, 250]
    );

    assert!(cipher::decrypt(&mut plain));

    assert_eq!(plain, [12, 0, 0, 16, 0, 0, 0, 0, 0, 0, 0, 0]);
}

#[test]
fn cipher_round_trips_all_keys_across_key_stream_wraps() {
    for key in 0..=u8::MAX {
        let mut plain: Vec<u8> = (0..525).map(|index| (index % 256) as u8).collect();

        plain[2] = key;

        plain[3] = cipher::checksum(key, plain.len());

        let mut encrypted = plain.clone();

        cipher::encrypt(&mut encrypted, key);

        assert_eq!(&encrypted[..4], &plain[..4]);

        assert!(cipher::decrypt(&mut encrypted), "key {key}");

        assert_eq!(encrypted, plain, "key {key}");
    }
}

#[test]
fn seal_sets_transport_fields_and_preserves_opcode_and_payload() {
    let bytes = frame(0x2345, b"synthetic");

    let mut decoder = Decoder::new();

    decoder.feed(&bytes);

    let packet = decoder.next_packet().unwrap().unwrap();

    assert_eq!(packet.header.size as usize, header::LEN + 9);

    assert_eq!(packet.header.key, 0x42);

    assert_eq!(packet.header.client_id, 0x1234);

    assert_eq!(packet.header.tick, 0x01020304);

    assert_eq!(packet.opcode(), 0x2345);

    assert_eq!(packet.payload, b"synthetic");

    assert!(packet.checksum_ok);

    assert_eq!(decoder.pending(), 0);
}

#[test]
fn decoder_handles_every_split_of_greeting_header_and_payload() {
    let mut wire = codec::HELLO.to_vec();

    wire.extend_from_slice(&frame(0x2345, b"synthetic"));

    for split in 0..wire.len() {
        let mut decoder = Decoder::expecting_hello();

        decoder.feed(&wire[..split]);

        assert!(decoder.next_packet().is_none(), "split {split}");

        decoder.feed(&wire[split..]);

        let packet = decoder.next_packet().unwrap().unwrap();

        assert_eq!(packet.opcode(), 0x2345);

        assert_eq!(packet.payload, b"synthetic");

        assert!(packet.checksum_ok);

        assert_eq!(decoder.pending(), 0);

        assert!(decoder.next_packet().is_none());
    }
}

#[test]
fn decoder_accepts_single_byte_feeds_and_multiple_frames() {
    let wire = [frame(1, b"first"), frame(2, b"second")].concat();

    let mut decoder = Decoder::new();

    let mut packets = Vec::new();

    for byte in wire {
        decoder.feed(&[byte]);

        if let Some(packet) = decoder.next_packet() {
            packets.push(packet.unwrap());
        }
    }

    assert_eq!(
        packets.iter().map(Packet::opcode).collect::<Vec<_>>(),
        [1, 2]
    );

    assert_eq!(packets[0].payload, b"first");

    assert_eq!(packets[1].payload, b"second");

    assert_eq!(decoder.pending(), 0);
}

#[test]
fn drain_preserves_a_partial_trailing_frame() {
    let first = frame(1, b"one");

    let second = frame(2, b"two");

    let third = frame(3, b"three");

    let mut decoder = Decoder::new();

    decoder.feed(&[first, second, third[..5].to_vec()].concat());

    let (packets, error) = decoder.drain();

    assert_eq!(error, None);

    assert_eq!(
        packets.iter().map(Packet::opcode).collect::<Vec<_>>(),
        [1, 2]
    );

    assert_eq!(decoder.pending(), 5);

    decoder.feed(&third[5..]);

    assert_eq!(decoder.next_packet().unwrap().unwrap().payload, b"three");
}

#[test]
fn decoder_rejects_invalid_greetings_without_interpreting_them_as_packets() {
    let mut decoder = Decoder::expecting_hello();

    decoder.feed(&[1, 2, 3]);

    assert!(decoder.next_packet().is_none());

    decoder.feed(&[4]);

    assert_eq!(
        decoder.next_packet(),
        Some(Err(DecodeError::BadHello(0x04030201)))
    );
}

#[test]
fn invalid_size_clears_buffer_and_allows_a_new_frame() {
    for size in [0_u16, 11, 10241, u16::MAX] {
        let mut decoder = Decoder::new();

        let mut invalid = [0; header::LEN];

        invalid[..2].copy_from_slice(&size.to_le_bytes());

        decoder.feed(&invalid);

        assert_eq!(decoder.next_packet(), Some(Err(DecodeError::BadSize(size))));

        assert_eq!(decoder.pending(), 0);

        decoder.feed(&frame(1, b"valid"));

        assert_eq!(decoder.next_packet().unwrap().unwrap().payload, b"valid");
    }
}

#[test]
fn decoder_accepts_the_minimum_and_maximum_frame_sizes() {
    for size in [header::MIN_SIZE, header::MAX_SIZE] {
        let payload = vec![0x5a; size - header::LEN];

        let mut decoder = Decoder::new();

        decoder.feed(&frame(1, &payload));

        let packet = decoder.next_packet().unwrap().unwrap();

        assert_eq!(packet.payload, payload);

        assert!(packet.checksum_ok);
    }
}

#[test]
fn drain_stops_at_a_checksum_mismatch_without_consuming_later_frames() {
    let mut invalid = frame(1, b"bad checksum field");

    invalid[header::CHECKSUM_OFF] ^= 1;

    let next = frame(2, b"next");

    let mut decoder = Decoder::new();

    decoder.feed(&invalid);

    decoder.feed(&next);

    let (packets, error) = decoder.drain();

    assert_eq!(error, None);

    assert_eq!(packets.len(), 1);

    assert!(!packets[0].checksum_ok);

    assert_eq!(decoder.pending(), next.len());

    assert_eq!(decoder.next_packet().unwrap().unwrap().opcode(), 2);
}

#[test]
fn drain_returns_valid_packets_before_a_size_error() {
    let mut decoder = Decoder::new();

    decoder.feed(&frame(1, b"valid"));

    decoder.feed(&[0; header::LEN]);

    let (packets, error) = decoder.drain();

    assert_eq!(packets.len(), 1);

    assert_eq!(packets[0].payload, b"valid");

    assert_eq!(error, Some(DecodeError::BadSize(0)));
}

#[test]
fn rng_preserves_the_expected_seed_sequence() {
    let mut rng = rng::Lcg::new(1);

    assert_eq!(
        std::array::from_fn::<_, 5, _>(|_| rng.next()),
        [41, 18467, 6334, 26500, 19169]
    );

    let mut object = rng::Lcg::for_object(3, 2);

    let mut seeded = rng::Lcg::new(51);

    for _ in 0..20 {
        assert_eq!(object.next(), seeded.next());
    }
}
