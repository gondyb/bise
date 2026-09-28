//! Copy to the system clipboard: pbcopy on a local macOS session (no
//! size limit, no terminal setting), else OSC 52 through the terminal
//! (Ghostty allows clipboard writes by default; works over ssh; tmux
//! needs `set-clipboard on`).

use std::io::Write;

/// Copies `text`; false when no way worked.
pub(crate) fn copy(text: &str) -> bool {
    let remote = std::env::var_os("SSH_TTY").is_some() || std::env::var_os("SSH_CONNECTION").is_some();
    if cfg!(target_os = "macos") && !remote && pbcopy(text) {
        return true;
    }
    osc52(text)
}

fn pbcopy(text: &str) -> bool {
    let child = std::process::Command::new("pbcopy")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn();
    let Ok(mut child) = child else { return false };
    let wrote = child.stdin.take().is_some_and(|mut i| i.write_all(text.as_bytes()).is_ok());
    child.wait().is_ok_and(|s| s.success()) && wrote
}

fn osc52(text: &str) -> bool {
    let mut out = std::io::stdout();
    write!(out, "\x1b]52;c;{}\x07", base64(text.as_bytes())).is_ok() && out.flush().is_ok()
}

/// Standard base64 with padding (the OSC 52 payload).
pub(crate) fn base64(data: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut s = String::with_capacity(data.len().div_ceil(3) * 4);
    for c in data.chunks(3) {
        let n = (c[0] as u32) << 16 | (*c.get(1).unwrap_or(&0) as u32) << 8 | *c.get(2).unwrap_or(&0) as u32;
        for i in 0..4 {
            if i <= c.len() {
                s.push(T[(n >> (18 - 6 * i) & 63) as usize] as char);
            } else {
                s.push('=');
            }
        }
    }
    s
}

#[cfg(test)]
mod tests {
    #[test]
    fn base64_matches_the_rfc_vectors() {
        let v = [("", ""), ("f", "Zg=="), ("fo", "Zm8="), ("foo", "Zm9v"), ("foob", "Zm9vYg=="), ("fooba", "Zm9vYmE="), ("foobar", "Zm9vYmFy")];
        for (i, o) in v {
            assert_eq!(super::base64(i.as_bytes()), o);
        }
    }
}
