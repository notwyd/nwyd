use crate::config::Language;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]

pub enum Codepage {
    #[default]
    Windows1252,
    Windows949,
    Windows950,
    Windows936,
    Windows1254,
    Windows1250,
    Unsupported(u16),
}

impl Codepage {
    pub fn for_language(language: Language) -> Codepage {
        match language {
            Language::English | Language::Portuguese | Language::Spanish => Codepage::Windows1252,
            Language::Korean => Codepage::Windows949,
            Language::TraditionalChinese => Codepage::Windows950,
            Language::SimplifiedChinese => Codepage::Windows936,
            Language::Turkish => Codepage::Windows1254,
            Language::Polish => Codepage::Windows1250,
        }
    }

    pub fn is_supported(&self) -> bool {
        !matches!(self, Codepage::Unsupported(_))
    }

    fn encoding(self) -> Option<&'static encoding_rs::Encoding> {
        match self {
            Codepage::Windows949 => Some(encoding_rs::EUC_KR),
            Codepage::Windows950 => Some(encoding_rs::BIG5),
            Codepage::Windows936 => Some(encoding_rs::GBK),
            Codepage::Windows1254 => Some(encoding_rs::WINDOWS_1254),
            Codepage::Windows1250 => Some(encoding_rs::WINDOWS_1250),
            Codepage::Windows1252 | Codepage::Unsupported(_) => None,
        }
    }
}

const CP1252_HIGH: [char; 32] = [
    '\u{20AC}', '\u{0081}', '\u{201A}', '\u{0192}', '\u{201E}', '\u{2026}', '\u{2020}', '\u{2021}',
    '\u{02C6}', '\u{2030}', '\u{0160}', '\u{2039}', '\u{0152}', '\u{008D}', '\u{017D}', '\u{008F}',
    '\u{0090}', '\u{2018}', '\u{2019}', '\u{201C}', '\u{201D}', '\u{2022}', '\u{2013}', '\u{2014}',
    '\u{02DC}', '\u{2122}', '\u{0161}', '\u{203A}', '\u{0153}', '\u{009D}', '\u{017E}', '\u{0178}',
];

pub fn decode(bytes: &[u8], codepage: Codepage) -> String {
    if let Some(encoding) = codepage.encoding() {
        return encoding.decode_without_bom_handling(bytes).0.into_owned();
    }

    bytes
        .iter()
        .map(|&b| match (b, codepage) {
            (0x80..=0x9F, Codepage::Windows1252) => CP1252_HIGH[(b - 0x80) as usize],
            _ => b as char,
        })
        .collect()
}

pub fn encode(text: &str, codepage: Codepage) -> Vec<u8> {
    if let Some(encoding) = codepage.encoding() {
        let mut encoder = encoding.new_encoder();

        let mut bytes = Vec::with_capacity(text.len());

        let mut buffer = [0; 1024];

        let mut input = text;

        loop {
            let (result, read, written) =
                encoder.encode_from_utf8_without_replacement(input, &mut buffer, true);

            bytes.extend_from_slice(&buffer[..written]);

            input = &input[read..];

            match result {
                encoding_rs::EncoderResult::InputEmpty => return bytes,
                encoding_rs::EncoderResult::OutputFull => {}
                encoding_rs::EncoderResult::Unmappable(_) => bytes.push(b'?'),
            }
        }
    }

    text.chars()
        .map(|c| {
            if codepage == Codepage::Windows1252
                && let Some(i) = CP1252_HIGH.iter().position(|&h| h == c)
            {
                return 0x80 + i as u8;
            }

            match c as u32 {
                v @ 0..=0xFF => v as u8,
                _ => b'?',
            }
        })
        .collect()
}

#[cfg(test)]

mod tests {

    use super::*;

    #[test]

    fn configured_languages_decode_known_bytes_and_round_trip() {
        let samples: [(Language, &str, &[u8]); 8] = [
            (Language::English, "Settings", b"Settings"),
            (Language::Portuguese, "Português", b"Portugu\xEAs"),
            (Language::Spanish, "Español", b"Espa\xF1ol"),
            (Language::Korean, "한국어", b"\xC7\xD1\xB1\xB9\xBE\xEE"),
            (
                Language::TraditionalChinese,
                "繁體中文",
                b"\xC1\x63\xC5\xE9\xA4\xA4\xA4\xE5",
            ),
            (
                Language::SimplifiedChinese,
                "简体中文",
                b"\xBC\xF2\xCC\xE5\xD6\xD0\xCE\xC4",
            ),
            (
                Language::Turkish,
                "Türkçe İıĞğŞş",
                b"T\xFCrk\xE7e \xDD\xFD\xD0\xF0\xDE\xFE",
            ),
            (
                Language::Polish,
                "Zażółć gęślą jaźń",
                b"Za\xBF\xF3\xB3\xE6 g\xEA\x9Cl\xB9 ja\x9F\xF1",
            ),
        ];

        for (language, text, bytes) in samples {
            let codepage = Codepage::for_language(language);

            assert!(codepage.is_supported(), "{language:?}");

            assert_eq!(decode(bytes, codepage), text, "{language:?}");

            assert_eq!(encode(text, codepage), bytes, "{language:?}");
        }
    }

    #[test]

    fn unmappable_characters_become_single_question_marks_after_buffer_boundaries() {
        for codepage in [
            Codepage::Windows949,
            Codepage::Windows950,
            Codepage::Windows936,
            Codepage::Windows1254,
            Codepage::Windows1250,
        ] {
            let text = format!("{}🦀😀Z", "abc".repeat(700));

            let expected = format!("{}??Z", "abc".repeat(700));

            assert_eq!(encode(&text, codepage), expected.as_bytes(), "{codepage:?}");

            assert!(encode("", codepage).is_empty());
        }
    }

    #[test]

    fn malformed_multibyte_sequences_are_replaced_without_losing_following_ascii() {
        for codepage in [
            Codepage::Windows949,
            Codepage::Windows950,
            Codepage::Windows936,
        ] {
            assert_eq!(decode(b"\x81 A\x81", codepage), "\u{FFFD} A\u{FFFD}");
        }
    }

    #[test]

    fn unknown_codepages_keep_the_existing_byte_preserving_fallback() {
        let codepage = Codepage::Unsupported(12345);

        let bytes: Vec<u8> = (0..=u8::MAX).collect();

        assert!(!codepage.is_supported());

        assert_eq!(encode(&decode(&bytes, codepage), codepage), bytes);

        assert_eq!(encode("A🦀", codepage), b"A?");
    }
}
