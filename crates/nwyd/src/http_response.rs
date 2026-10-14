#[cfg(not(target_arch = "wasm32"))]
use std::io;

#[cfg(not(target_arch = "wasm32"))]

pub(crate) fn read_bounded(
    limit: usize,
    once: bool,
    mut read: impl FnMut(&mut [u8]) -> io::Result<usize>,
) -> Vec<u8> {
    let mut bytes = vec![0; limit];

    let mut total = 0;

    while total < limit {
        let Ok(count) = read(&mut bytes[total..]) else {
            break;
        };

        if count == 0 || count > limit - total {
            break;
        }

        total += count;

        if once {
            break;
        }
    }

    bytes.truncate(total);

    bytes
}

pub(crate) fn c_body(bytes: &[u8], capacity: usize) -> &[u8] {
    let end = bytes
        .iter()
        .position(|&byte| byte == 0)
        .unwrap_or(bytes.len());

    &bytes[..end.min(capacity.saturating_sub(1))]
}

#[cfg(test)]

mod tests {

    use super::*;
    use std::io::Read;

    #[test]

    fn fragmented_reads_fill_but_never_exceed_limit() {
        let mut input = &b"abcdefgh"[..];

        let mut calls = 0;

        let body = read_bounded(5, false, |buffer| {
            calls += 1;

            let count = buffer.len().min(2);

            input.read(&mut buffer[..count])
        });

        assert_eq!(body, b"abcde");

        assert_eq!(input, b"fgh");

        assert_eq!(calls, 3);
    }

    #[test]

    fn single_read_mode_stops_after_first_chunk() {
        let mut input = &b"abcdef"[..];

        let mut calls = 0;

        let body = read_bounded(6, true, |buffer| {
            calls += 1;

            input.read(&mut buffer[..2])
        });

        assert_eq!(body, b"ab");

        assert_eq!(input, b"cdef");

        assert_eq!(calls, 1);
    }

    #[test]

    fn eof_and_read_errors_preserve_only_the_successful_prefix() {
        for fail in [false, true] {
            let mut calls = 0;

            let body = read_bounded(8, false, |buffer| {
                calls += 1;

                if calls == 1 {
                    buffer[..3].copy_from_slice(b"abc");

                    Ok(3)
                } else if fail {
                    Err(io::Error::new(
                        io::ErrorKind::ConnectionReset,
                        "synthetic read failure",
                    ))
                } else {
                    Ok(0)
                }
            });

            assert_eq!(body, b"abc");

            assert_eq!(calls, 2);
        }
    }

    #[test]

    fn empty_capacity_never_invokes_reader() {
        let body = read_bounded(0, false, |_| panic!("empty response cannot be read"));

        assert!(body.is_empty());
    }

    #[test]

    fn invalid_read_counts_do_not_commit_unread_bytes() {
        let mut calls = 0;

        let body = read_bounded(8, false, |buffer| {
            calls += 1;

            if calls == 1 {
                buffer[..2].copy_from_slice(b"ok");

                Ok(2)
            } else {
                Ok(buffer.len() + 1)
            }
        });

        assert_eq!(body, b"ok");

        assert_eq!(calls, 2);
    }

    #[test]

    fn c_body_reserves_space_and_stops_at_first_zero() {
        for (bytes, capacity, expected) in [
            (&b"hello"[..], 0, &b""[..]),
            (&b"hello"[..], 1, &b""[..]),
            (&b"hello"[..], 5, &b"hell"[..]),
            (&b"hello"[..], 6, &b"hello"[..]),
            (&b"ab\0ignored"[..], 20, &b"ab"[..]),
            (&b"ab\0ignored"[..], 2, &b"a"[..]),
            (&b"\0ignored"[..], 20, &b""[..]),
            (&b""[..], usize::MAX, &b""[..]),
        ] {
            assert_eq!(c_body(bytes, capacity), expected);
        }
    }
}
