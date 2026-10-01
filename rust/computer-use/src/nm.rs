//! Chrome native messaging frames: a 32-bit length in native byte order
//! (little-endian on every Mac), then that many bytes of UTF-8 JSON.

use std::io::{Read, Write};

use serde_json::Value;

/// Chrome refuses a message from the host above 1 MB.
pub const MAX_TO_BROWSER: usize = 1024 * 1024;
/// Messages from the extension (screenshots) may be large; above this is a broken stream.
pub const MAX_FROM_BROWSER: usize = 64 * 1024 * 1024;

/// One message; `Ok(None)` at a clean end of stream.
pub fn read(r: &mut impl Read) -> std::io::Result<Option<Value>> {
    let mut len = [0u8; 4];
    match r.read_exact(&mut len) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(e) => return Err(e),
    }
    let n = u32::from_ne_bytes(len) as usize;
    if n > MAX_FROM_BROWSER {
        return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, format!("frame of {} bytes", n)));
    }
    let mut b = vec![0u8; n];
    r.read_exact(&mut b)?;
    serde_json::from_slice(&b).map(Some).map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
}

pub fn write(w: &mut impl Write, v: &Value) -> std::io::Result<()> {
    let b = v.to_string();
    if b.len() > MAX_TO_BROWSER {
        return Err(std::io::Error::new(std::io::ErrorKind::InvalidInput, "message above Chrome's 1 MB"));
    }
    w.write_all(&(b.len() as u32).to_ne_bytes())?;
    w.write_all(b.as_bytes())?;
    w.flush()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn frames() {
        let mut buf = Vec::new();
        write(&mut buf, &json!({"hello": {"browser": "chrome"}})).unwrap();
        write(&mut buf, &json!({"id": 1})).unwrap();
        assert_eq!(&buf[..4], &(30u32).to_le_bytes());
        let mut r = &buf[..];
        assert_eq!(read(&mut r).unwrap(), Some(json!({"hello": {"browser": "chrome"}})));
        assert_eq!(read(&mut r).unwrap(), Some(json!({"id": 1})));
        assert_eq!(read(&mut r).unwrap(), None);
    }
}
