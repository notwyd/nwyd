use std::{
    net::{IpAddr, SocketAddr},
    time::Duration,
};

pub(crate) const MAX_MESSAGE_BYTES: usize = 4 * 1024 * 1024;
pub(crate) const MAX_HTTP_REQUEST_BYTES: usize = 64 * 1024;
pub(crate) const MAX_HTTP_RESPONSE_BYTES: usize = 4 * 1024 * 1024;

pub(crate) fn token_matches(expected: &str, actual: &str) -> bool {
    expected.len() == actual.len()
        && expected
            .bytes()
            .zip(actual.bytes())
            .fold(0u8, |diff, (a, b)| diff | (a ^ b))
            == 0
}

pub(crate) fn valid_host_header(host: &str, port: u16) -> bool {
    host == format!("127.0.0.1:{port}")
        || host == format!("localhost:{port}")
        || host == format!("[::1]:{port}")
}

pub(crate) fn is_loopback_origin(origin: &str) -> bool {
    let Ok(url) = url::Url::parse(origin) else {
        return false;
    };
    if !matches!(url.scheme(), "http" | "https") || url.origin().ascii_serialization() != origin {
        return false;
    }
    match url.host() {
        Some(url::Host::Domain("localhost")) => true,
        Some(url::Host::Ipv4(address)) => address.is_loopback(),
        Some(url::Host::Ipv6(address)) => address.is_loopback(),
        _ => false,
    }
}

pub(crate) fn is_public_address(address: IpAddr) -> bool {
    match address {
        IpAddr::V4(address) => {
            let [a, b, c, _] = address.octets();
            !(a == 0
                || a == 10
                || a == 127
                || a >= 224
                || (a == 100 && (64..=127).contains(&b))
                || (a == 169 && b == 254)
                || (a == 172 && (16..=31).contains(&b))
                || (a == 192 && (b == 168 || (b == 0 && (c == 0 || c == 2))))
                || (a == 198 && (b == 18 || b == 19 || (b == 51 && c == 100)))
                || (a == 203 && b == 0 && c == 113))
        }
        IpAddr::V6(address) => {
            if let Some(mapped) = address.to_ipv4_mapped() {
                return is_public_address(IpAddr::V4(mapped));
            }
            let segments = address.segments();
            (segments[0] & 0xe000) == 0x2000
                && !(segments[0] == 0x2001 && (segments[1] < 0x0200 || segments[1] == 0x0db8))
                && segments[0] != 0x2002
                && !(segments[0] == 0x3fff && segments[1] < 0x1000)
        }
    }
}

pub(crate) async fn resolve_target(
    host: &str,
    port: u16,
    allow_private: bool,
) -> Result<Vec<SocketAddr>, String> {
    if host.is_empty()
        || host.len() > 253
        || port == 0
        || host.bytes().any(|b| {
            b.is_ascii_whitespace() || matches!(b, b'/' | b'\\' | b'@' | b'%' | b'?' | b'#')
        })
    {
        return Err("Endereço do servidor inválido.".into());
    }
    let host = host
        .strip_prefix('[')
        .and_then(|s| s.strip_suffix(']'))
        .unwrap_or(host);
    let addresses: Vec<_> = tokio::time::timeout(
        Duration::from_secs(5),
        tokio::net::lookup_host((host, port)),
    )
    .await
    .map_err(|_| "O tempo para localizar o servidor esgotou.")?
    .map_err(|_| "Não foi possível localizar o servidor do jogo.")?
    .collect();
    if addresses.is_empty() || addresses.len() > 64 {
        return Err("Nenhum endereço válido foi encontrado para o servidor.".into());
    }
    if !allow_private
        && addresses
            .iter()
            .any(|address| !is_public_address(address.ip()))
    {
        return Err("O relay só se conecta a servidores públicos do jogo.".into());
    }
    Ok(addresses)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_non_public_destinations_including_mapped_ipv6() {
        for address in [
            "127.0.0.1",
            "10.2.3.4",
            "172.31.2.3",
            "192.168.1.1",
            "169.254.169.254",
            "100.100.100.200",
            "0.0.0.0",
            "224.0.0.1",
            "192.0.2.1",
            "198.18.0.1",
            "::1",
            "fc00::1",
            "fe80::1",
            "::ffff:127.0.0.1",
            "2001:db8::1",
            "2002:7f00:1::",
        ] {
            assert!(!is_public_address(address.parse().unwrap()), "{address}");
        }
        for address in [
            "1.1.1.1",
            "8.8.8.8",
            "2001:4860:4860::8888",
            "2606:4700:4700::1111",
            "::ffff:8.8.8.8",
        ] {
            assert!(is_public_address(address.parse().unwrap()), "{address}");
        }
    }

    #[test]
    fn host_and_token_checks_are_exact() {
        assert!(valid_host_header("127.0.0.1:8787", 8787));
        assert!(valid_host_header("localhost:8787", 8787));
        assert!(!valid_host_header("evil.example:8787", 8787));
        assert!(!valid_host_header("127.0.0.1:80", 8787));
        assert!(!valid_host_header("127.0.0.1:8787.evil.example", 8787));
        assert!(token_matches("abcd", "abcd"));
        assert!(!token_matches("abcd", "abce"));
        assert!(!token_matches("abcd", "abc"));
    }
}
