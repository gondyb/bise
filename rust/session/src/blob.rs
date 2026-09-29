//! The content-addressed blob store (§5.1): ~/.bise/blobs/sha256/<2>/<62>.
//! A blob is written before the event that refers to it: temp file,
//! fsync, rename (§8.1). Same bytes, same file: written once.
use crate::types::BlobRef;
use std::io::Write;
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::path::{Path, PathBuf};

pub fn sha256_hex(bytes: &[u8]) -> String {
    let d = ring::digest::digest(&ring::digest::SHA256, bytes);
    d.as_ref().iter().map(|b| format!("{b:02x}")).collect()
}

/// The file of a blob.
pub fn path(blobs: &Path, sha256: &str) -> PathBuf {
    blobs.join("sha256").join(&sha256[..2.min(sha256.len())]).join(sha256.get(2..).unwrap_or(""))
}

pub fn mkdir_private(dir: &Path) -> std::io::Result<()> {
    std::fs::DirBuilder::new().recursive(true).mode(0o700).create(dir)
}

/// Put bytes in the store; the reference to write in the event.
pub fn put(blobs: &Path, bytes: &[u8], mime: &str) -> std::io::Result<BlobRef> {
    let sha = sha256_hex(bytes);
    let file = path(blobs, &sha);
    if !file.exists() {
        let dir = file.parent().expect("a blob has a folder");
        mkdir_private(dir)?;
        let tmp = dir.join(format!(".{}.tmp.{}", &sha[2..], std::process::id()));
        let mut f = std::fs::OpenOptions::new().write(true).create(true).truncate(true).mode(0o600).open(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()?;
        std::fs::rename(&tmp, &file)?;
        if let Ok(d) = std::fs::File::open(dir) {
            let _ = d.sync_all();
        }
    }
    Ok(BlobRef { sha256: sha, bytes: bytes.len() as u64, mime: mime.to_string() })
}

/// The bytes of a blob, checked against its hash.
pub fn get(blobs: &Path, r: &BlobRef) -> std::io::Result<Vec<u8>> {
    let b = std::fs::read(path(blobs, &r.sha256))?;
    if sha256_hex(&b) != r.sha256 {
        return Err(std::io::Error::other(format!("blob {} does not match its hash", r.sha256)));
    }
    Ok(b)
}
