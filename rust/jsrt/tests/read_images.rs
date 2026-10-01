// self.readImages / self.readImage: the built-in that turns local image
// files into the image blocks of a program's result. The bend-jsrt binary
// runs real programs here (one round, no tool results): a file that is
// there becomes an image marker, a missing file or another type fails the
// program with the reason, and no call ever goes out as a tool request.

use std::path::{Path, PathBuf};
use std::process::Command;

const PNG_1X1: &[u8] = &[
    0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1f, 0x15, 0xc4,
    0x89, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9c, 0x63, 0xf8, 0xcf, 0xc0, 0xf0,
    0x1f, 0x00, 0x05, 0x00, 0x01, 0xff, 0x89, 0x99, 0x3d, 0x1d, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45,
    0x4e, 0x44, 0xae, 0x42, 0x60, 0x82,
];

// a fresh folder per test: the working dir (BEND_WORKDIR), the image
// store (BEND_IMAGE_DIR), the program files
fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("jsrt-read-images-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("shots")).unwrap();
    std::fs::write(dir.join("shots/a.png"), PNG_1X1).unwrap();
    std::fs::write(dir.join("shots/b.png"), PNG_1X1).unwrap();
    std::fs::write(dir.join("notes.png"), "not an image, only text").unwrap();
    dir
}

// run one program: (exit code, stdout)
fn run(dir: &Path, program: &str) -> (i32, String) {
    let prog = dir.join("prog.ts");
    let results = dir.join("results.json");
    std::fs::write(&prog, program).unwrap();
    std::fs::write(&results, "[]").unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_bend-jsrt"))
        .arg(&prog)
        .arg(&results)
        .env("BEND_WORKDIR", dir)
        .env("BEND_IMAGE_DIR", dir.join("store"))
        .current_dir(std::env::temp_dir())
        .output()
        .unwrap();
    (out.status.code().unwrap_or(-1), String::from_utf8_lossy(&out.stdout).to_string())
}

#[test]
fn read_images_returns_the_blocks_of_existing_files() {
    let dir = scratch("ok");
    // relative paths are the working dir's; an array gives one block each
    let (code, out) = run(&dir, "async function main() { return self.readImages(['shots/a.png', 'shots/b.png']); }");
    assert_eq!(code, 0, "{out}");
    let a = dir.join("shots/a.png");
    let b = dir.join("shots/b.png");
    assert!(out.contains(&format!("<image name=\"[Image #1]\" path=\"{}\" mime=\"image/png\"", a.display())), "{out}");
    assert!(out.contains(&format!("<image name=\"[Image #2]\" path=\"{}\" mime=\"image/png\"", b.display())), "{out}");
    // one path (absolute), the alias, mixed with a text block
    let program = format!(
        "return [{{ type: 'text', text: 'before' }}, ...self.readImage('{}')];",
        a.display()
    );
    let (code, out) = run(&dir, &program);
    assert_eq!(code, 0, "{out}");
    assert!(out.starts_with("before
<image name=\"[Image #1]\""), "{out}");
    // the blocks themselves, as a program sees them
    let (code, out) = run(&dir, "return JSON.stringify(self.readImages('shots/a.png'));");
    assert_eq!(code, 0, "{out}");
    assert_eq!(out.trim(), format!("[{{\"type\":\"image\",\"path\":\"{}\"}}]", a.display()));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn read_images_fails_on_a_missing_file() {
    let dir = scratch("missing");
    let (code, out) = run(&dir, "return self.readImages(['shots/a.png', 'shots/nope.png']);");
    assert_eq!(code, 43, "{out}");
    let want = format!("self.readImages: no file at {}", dir.join("shots/nope.png").display());
    assert!(out.contains(&want), "{out}");
    // a directory is no file either
    let (code, out) = run(&dir, "return self.readImages('shots');");
    assert_eq!(code, 43, "{out}");
    assert!(out.contains("is not a file"), "{out}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn read_images_fails_on_another_type() {
    let dir = scratch("type");
    // the content decides, not the extension
    let (code, out) = run(&dir, "return self.readImage('notes.png');");
    assert_eq!(code, 43, "{out}");
    let want = format!("self.readImages: {} is not a PNG, JPEG, GIF or WebP image", dir.join("notes.png").display());
    assert!(out.contains(&want), "{out}");
    // not a path at all
    let (code, out) = run(&dir, "return self.readImages([]);");
    assert_eq!(code, 43, "{out}");
    assert!(out.contains("give a file path or an array of file paths"), "{out}");
    // a program can catch it and go on
    let (code, out) = run(&dir, "try { self.readImages('gone.png'); } catch (e) { return 'caught: ' + e.message; }");
    assert_eq!(code, 0, "{out}");
    assert!(out.starts_with("caught: self.readImages: no file at"), "{out}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn other_self_functions_stay_tool_calls() {
    let dir = scratch("tools");
    // self.compact still goes out as a tool request (exit 42)
    let (code, out) = run(&dir, "await self.compact(); return 'x';");
    assert_eq!(code, 42, "{out}");
    assert!(out.contains("\"tool\": \"self.compact\""), "{out}");
    let _ = std::fs::remove_dir_all(&dir);
}
