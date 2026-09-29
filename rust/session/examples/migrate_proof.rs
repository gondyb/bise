//! BISE-197 proof: migrate every given BEND-SESSION file into <out>/sessions
//! (blobs in <out>/blobs), each checked by the byte round trip; prints one
//! line per file and the totals. Never writes next to the inputs.
use bise_session::migrate::{migrate_txt, Outcome, Source};
use std::path::PathBuf;

fn main() {
    let mut args = std::env::args().skip(1);
    let out = PathBuf::from(args.next().expect("usage: migrate_proof <out> <file>..."));
    let (mut ok, mut unloadable, mut failed, mut msgs, mut dropped, mut images) = (0, 0, 0, 0, 0, 0);
    for f in args {
        let src = Source { txt: f.clone().into(), cwd: String::new(), agent: None, model: None };
        match migrate_txt(&src, &out.join("sessions"), &out.join("blobs"), "bise-proof") {
            Outcome::Migrated { id, messages, dropped_lines, images: n, .. } => {
                ok += 1;
                msgs += messages;
                dropped += dropped_lines;
                images += n;
                println!("ok         {f} -> {id} ({messages} messages, {dropped_lines} skipped lines, {n} images)");
            }
            Outcome::Unloadable => {
                unloadable += 1;
                println!("unloadable {f}");
            }
            Outcome::Failed(why) => {
                failed += 1;
                println!("FAILED     {f}: {why}");
            }
        }
    }
    println!("TOTAL migrated {ok} (messages {msgs}, skipped lines {dropped}, images {images}), unloadable {unloadable}, failed {failed}");
}
