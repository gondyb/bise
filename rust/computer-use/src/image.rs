//! Screenshots (brief item 5): the base64 JPEG from the extension or the
//! helper becomes a file in the agent's `TMPDIR`, at most `max_width`
//! wide. The width comes from the JPEG's frame header; a wider picture is
//! scaled with macOS's `sips` (no image crate in the tree).

use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use crate::proto::err;

/// `(width, height)` of a JPEG, from its first SOFn marker.
pub fn jpeg_size(b: &[u8]) -> Option<(u32, u32)> {
    if b.len() < 4 || b[0] != 0xFF || b[1] != 0xD8 {
        return None;
    }
    let mut i = 2;
    while i + 4 <= b.len() {
        if b[i] != 0xFF {
            i += 1;
            continue;
        }
        let m = b[i + 1];
        if m == 0xFF {
            i += 1;
            continue;
        }
        // markers without a length
        if m == 0x01 || (0xD0..=0xD9).contains(&m) {
            i += 2;
            continue;
        }
        let len = (b[i + 2] as usize) << 8 | b[i + 3] as usize;
        let sof = matches!(m, 0xC0..=0xCF) && !matches!(m, 0xC4 | 0xC8 | 0xCC);
        if sof && i + 9 <= b.len() {
            let h = (b[i + 5] as u32) << 8 | b[i + 6] as u32;
            let w = (b[i + 7] as u32) << 8 | b[i + 8] as u32;
            return Some((w, h));
        }
        i += 2 + len;
    }
    None
}

/// Scale `path` in place to `max_width` wide (`sips`, macOS).
fn scale(path: &Path, max_width: u32) -> Result<(), String> {
    let out = std::process::Command::new("sips")
        .arg("--resampleWidth")
        .arg(max_width.to_string())
        .arg(path)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .output()
        .map_err(|e| format!("sips: {}", e))?;
    if out.status.success() {
        Ok(())
    } else {
        Err(format!("sips: {}", String::from_utf8_lossy(&out.stderr).trim()))
    }
}

/// A file name for one screenshot of `target` (`tab:17` -> `computer-tab-17-<n>.jpg`).
fn file_name(target: &str, n: u64) -> String {
    let t: String = target.chars().map(|c| if c.is_ascii_alphanumeric() || c == '.' { c } else { '-' }).collect();
    format!("computer-{}-{}.jpg", t, n)
}

/// The C1 `screenshot` result from a C4/C5 one (`{data, mime, width, height}`):
/// the JPEG written in `dir`, scaled down to `max_width`.
pub fn write_screenshot(dir: &Path, target: &str, n: u64, res: &Value, max_width: u32) -> Result<Value, Value> {
    let data = res.get("data").and_then(Value::as_str).ok_or_else(|| err("timeout", "the screenshot came back empty; try again"))?;
    let bytes = crate::b64::decode(data).map_err(|e| err("timeout", format!("the screenshot came back broken ({}); try again", e)))?;
    std::fs::create_dir_all(dir).map_err(|e| err("bad_args", format!("cannot write in {}: {}", dir.display(), e)))?;
    let path: PathBuf = dir.join(file_name(target, n));
    std::fs::write(&path, &bytes).map_err(|e| err("bad_args", format!("cannot write {}: {}", path.display(), e)))?;
    let mut size = jpeg_size(&bytes);
    if let Some((w, _)) = size {
        if max_width > 0 && w > max_width && scale(&path, max_width).is_ok() {
            size = std::fs::read(&path).ok().and_then(|b| jpeg_size(&b));
        }
    }
    let num = |k: &str| res.get(k).and_then(Value::as_u64).unwrap_or(0) as u32;
    let (w, h) = size.unwrap_or((num("width"), num("height")));
    Ok(json!({"path": path, "mime": "image/jpeg", "width": w, "height": h}))
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A `w`x`h` JPEG made with `sips` from a BMP (tests only).
    pub(crate) fn jpeg(dir: &Path, w: u32, h: u32) -> Vec<u8> {
        let row = (w * 3).div_ceil(4) * 4;
        let size = 54 + row * h;
        let mut b = Vec::with_capacity(size as usize);
        b.extend_from_slice(b"BM");
        for v in [size, 0, 54, 40] {
            b.extend_from_slice(&v.to_le_bytes());
        }
        b.extend_from_slice(&(w as i32).to_le_bytes());
        b.extend_from_slice(&(h as i32).to_le_bytes());
        b.extend_from_slice(&1u16.to_le_bytes());
        b.extend_from_slice(&24u16.to_le_bytes());
        for v in [0u32, row * h, 2835, 2835, 0, 0] {
            b.extend_from_slice(&v.to_le_bytes());
        }
        for y in 0..h {
            for x in 0..row {
                b.push(((x + y) % 251) as u8);
            }
        }
        std::fs::create_dir_all(dir).unwrap();
        let (bmp, jpg) = (dir.join("in.bmp"), dir.join("out.jpg"));
        std::fs::write(&bmp, &b).unwrap();
        let ok = std::process::Command::new("sips")
            .args(["-s", "format", "jpeg"])
            .arg(&bmp)
            .arg("--out")
            .arg(&jpg)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .unwrap()
            .success();
        assert!(ok, "sips");
        std::fs::read(&jpg).unwrap()
    }

    #[test]
    fn writes_and_scales() {
        let dir = std::env::temp_dir().join(format!("cu-img-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let big = jpeg(&dir.join("src"), 1600, 90);
        assert_eq!(jpeg_size(&big), Some((1600, 90)));
        let res = json!({"data": crate::b64::encode(&big), "mime": "image/jpeg", "width": 1600, "height": 90});
        let out = write_screenshot(&dir.join("tmp"), "tab:17", 1, &res, 1280).unwrap();
        assert_eq!(out["width"], 1280);
        assert_eq!(out["height"], 72);
        let p = PathBuf::from(out["path"].as_str().unwrap());
        assert!(p.ends_with("computer-tab-17-1.jpg"), "{}", p.display());
        assert_eq!(jpeg_size(&std::fs::read(&p).unwrap()), Some((1280, 72)));
        let small = write_screenshot(&dir.join("tmp"), "app:com.apple.TextEdit", 2, &res, 4000).unwrap();
        assert_eq!(small["width"], 1600);
        assert!(write_screenshot(&dir, "tab:1", 3, &json!({}), 1280).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
