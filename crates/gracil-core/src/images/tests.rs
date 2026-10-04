use super::*;

#[test]
fn extensions_follow_the_format() {
    let ext = |name: &str, target| output_extension(Path::new(name), target);
    assert_eq!(ext("Foto.JPEG", Target::Jpeg), "JPEG");
    assert_eq!(ext("Foto.jfif", Target::Jpeg), "jfif");
    assert_eq!(ext("Foto.heic", Target::Jpeg), "jpg");
    assert_eq!(ext("Foto.webp", Target::Png), "png");
    assert_eq!(ext("Foto.PNG", Target::Png), "PNG");
}

#[test]
fn formats_are_known_by_their_first_bytes() {
    assert_eq!(Format::sniff(&[0xff, 0xd8, 0xff, 0xe0]), Format::Jpeg);
    assert_eq!(Format::sniff(b"\x89PNG\r\n\x1a\n...."), Format::Png);
    assert_eq!(Format::sniff(b"\0\0\0\x18ftypheic"), Format::Heif);
    assert_eq!(Format::sniff(b"RIFF\0\0\0\0WEBPVP8 "), Format::WebP);
    assert_eq!(Format::sniff(b"II*\0...."), Format::Other);
    assert_eq!(Format::sniff(b"BM......"), Format::Other);
    assert_eq!(Format::sniff(b"GIF89a.."), Format::NotAccepted);
    assert_eq!(
        Format::sniff(b"\0\0\0\x0cJXL \r\n\x87\n"),
        Format::NotAccepted
    );
    assert_eq!(Format::sniff(b"hello"), Format::Unknown);
}
