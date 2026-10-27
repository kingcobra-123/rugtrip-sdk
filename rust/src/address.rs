fn exact(s: &str) -> Option<&str> {
    let h = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X"))?;
    if (h.len() == 40 || h.len() == 64) && h.bytes().all(|b| b.is_ascii_hexdigit()) {
        Some(s)
    } else {
        None
    }
}

fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let (Some(h), Some(l)) = ((b[i + 1] as char).to_digit(16), (b[i + 2] as char).to_digit(16)) {
                out.push((h * 16 + l) as u8);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8(out).unwrap_or_else(|_| s.to_string())
}

/// Percent-encode everything except RFC 3986 unreserved characters.
pub(crate) fn percent_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// Token address, pool address, v4 pool id or chart link -> the path segment the API expects.
///
/// Links are reduced to the last 40- or 64-hex segment of their path; anything else is sent percent-encoded (the
/// server answers `bad_address`). Case is kept so the server can check EIP-55 checksums.
pub fn to_target(input: &str) -> String {
    let s = input.trim();
    if let Some(x) = exact(s) {
        return x.to_string();
    }
    if s.contains('/') {
        let path = s.split(['?', '#']).next().unwrap_or("");
        for seg in path.rsplit('/') {
            let d = percent_decode(seg);
            if let Some(x) = exact(&d) {
                return x.to_string();
            }
        }
    }
    percent_encode(s)
}
