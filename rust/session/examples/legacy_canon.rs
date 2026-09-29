//! Prints what the Core's loader keeps of a BEND-SESSION file (to_text of
//! parse), for a diff against the Bend loader (BISE-197 proof).
fn main() {
    let path = std::env::args().nth(1).expect("usage: legacy_canon <file>");
    let s = std::fs::read_to_string(&path).expect("read");
    match bise_session::legacy::parse(&s) {
        Some(p) => print!("{}", bise_session::legacy::to_text(&p)),
        None => print!("NONE"),
    }
}
