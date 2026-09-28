use super::*;

/// A valid 1x1 PNG.
pub(crate) const PNG_1X1: &str =
    "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==";

fn tmpdir(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("bend-images-test-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).expect("tmp dir");
    d
}

#[test]
fn base64_roundtrips() {
    for n in 0..40usize {
        let data: Vec<u8> = (0..n).map(|i| (i * 37 % 256) as u8).collect();
        assert_eq!(base64_decode(&base64_encode(&data)), Some(data));
    }
    assert_eq!(base64_decode("data:image/png;base64,QUJD"), Some(b"ABC".to_vec()));
    assert_eq!(base64_decode("QU*D"), None);
}

#[test]
fn sniff_and_dimensions() {
    let png = base64_decode(PNG_1X1).expect("png");
    assert_eq!(sniff(&png), Some(Kind::Png));
    assert_eq!(dimensions(&png), Some((1, 1)));
    let gif = b"GIF89a\x10\x00\x20\x00".to_vec();
    assert_eq!(dimensions(&gif), Some((16, 32)));
    // a minimal JPEG header: SOI, APP0 (len 4), SOF0 h=300 w=200
    let jpg = [
        0xff, 0xd8, 0xff, 0xe0, 0x00, 0x04, 0x00, 0x00, 0xff, 0xc0, 0x00, 0x11, 0x08, 0x01, 0x2c, 0x00,
        0xc8, 0x03,
    ];
    assert_eq!(sniff(&jpg), Some(Kind::Jpeg));
    assert_eq!(dimensions(&jpg), Some((200, 300)));
    assert_eq!(sniff(b"hello"), None);
    // truncated headers never panic
    for k in 0..png.len() {
        let _ = dimensions(&png[..k]);
    }
    for k in 0..jpg.len() {
        let _ = dimensions(&jpg[..k]);
    }
    let _ = dimensions(b"RIFF\0\0\0\0WEBPVP8X");
}

#[test]
fn store_marker_parse_display() {
    let d = tmpdir("store");
    std::env::set_var("BEND_IMAGE_DIR", &d);
    let png = base64_decode(PNG_1X1).expect("png");
    let s = store_bytes(png.clone()).expect("stored");
    assert_eq!(s.kind, Kind::Png);
    assert_eq!(std::fs::read(&s.file).expect("file"), png);
    assert_eq!(std::fs::read_to_string(&s.b64).expect("b64"), base64_encode(&png));
    let m = marker("[Image #1]", "shots/a \"b\">.png", &s);
    assert!(!m.contains('\n'));
    let text = format!("look {m} and <image name=\"x\"> broken, then {m}");
    let ms = markers(&text);
    assert_eq!(ms.len(), 2);
    assert_eq!(ms[0].name, "[Image #1]");
    assert_eq!(ms[0].path, "shots/a 'b').png");
    assert_eq!(ms[0].mime, "image/png");
    assert_eq!(ms[0].b64, s.b64.to_string_lossy());
    assert_eq!(
        display(&text),
        "look [Image #1 shots/a 'b').png] and <image name=\"x\"> broken, then [Image #1 shots/a 'b').png]"
    );
    assert!(store_bytes(b"not an image".to_vec()).is_err());
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn markers_never_panic_on_odd_text() {
    for t in ["", "<image name=\"", "<image name=\"é\" path=\"", "ééé<image name=\"a\" path=\"b\" mime=\"c\" b64=\"d\">é"] {
        let _ = markers(t);
        let _ = display(t);
    }
    let ms = markers("é<image name=\"a\" path=\"b\" mime=\"c\" b64=\"d\">é");
    assert_eq!(ms.len(), 1);
}

#[test]
fn pasted_paths_forms() {
    let d = tmpdir("paste");
    let png = base64_decode(PNG_1X1).expect("png");
    let spaced = d.join("Screen Shot 1.png");
    std::fs::write(&spaced, &png).expect("write");
    let plain = d.join("b.png");
    std::fs::write(&plain, &png).expect("write");
    let txt = d.join("c.png");
    std::fs::write(&txt, b"text").expect("write");
    let sp = spaced.to_string_lossy().to_string();
    // Finder drop: backslash-escaped spaces
    let esc = sp.replace(' ', "\\ ");
    assert_eq!(pasted_images(&format!("{esc} ")), Some(vec![spaced.clone()]));
    assert_eq!(pasted_images(&format!("'{sp}'")), Some(vec![spaced.clone()]));
    assert_eq!(pasted_images(&sp), Some(vec![spaced.clone()]));
    let url = format!("file://{}", sp.replace(' ', "%20"));
    assert_eq!(pasted_images(&url), Some(vec![spaced.clone()]));
    let two = format!("{esc} {}", plain.display());
    assert_eq!(pasted_images(&two), Some(vec![spaced, plain]));
    // not an image, not a file, plain text
    assert_eq!(pasted_images(&txt.to_string_lossy()), None);
    assert_eq!(pasted_images("hello world"), None);
    assert_eq!(pasted_images("'unclosed"), None);
    assert_eq!(pasted_images(""), None);
    assert!(has_image_ext("a/B.PNG") && has_image_ext("x.jpeg") && !has_image_ext("x.txt"));
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn big_image_is_downscaled_or_refused() {
    // a PNG header that claims 5000x10: prepare tries sips, which fails
    // on this fake body; the result is a clear error, never a panic
    let d = tmpdir("big");
    let mut png = base64_decode(PNG_1X1).expect("png");
    png[16..20].copy_from_slice(&5000u32.to_be_bytes());
    let r = prepare(png, &d);
    assert!(r.is_err());
    let _ = std::fs::remove_dir_all(&d);
}
