//! Carrying a photo's information into the JPEG jpegli writes.
//!
//! jpegli keeps the color profile of a JPEG it reads, but not its EXIF data (date taken, camera,
//! location, orientation), its XMP data or its IPTC captions. Gracil copies those segments from
//! the original into the result itself, right after the result's opening segments (ADR 0034).

const SOI: u8 = 0xd8;
const SOS: u8 = 0xda;
const APP0: u8 = 0xe0;
const APP1: u8 = 0xe1;
const APP2: u8 = 0xe2;
const APP13: u8 = 0xed;

const EXIF: &[u8] = b"Exif\0\0";
const XMP: &[u8] = b"http://ns.adobe.com/xap/1.0/\0";
const ICC: &[u8] = b"ICC_PROFILE\0";
/// The most a segment can hold, its 2-byte length included.
const MAX_SEGMENT: usize = 0xffff;

/// One segment before the picture data: its marker and what follows its length.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Segment<'a> {
    marker: u8,
    payload: &'a [u8],
}

/// The segments before the picture data, or `None` if `data` is not a JPEG.
fn segments(data: &[u8]) -> Option<Vec<Segment<'_>>> {
    if data.get(..2)? != [0xff, SOI] {
        return None;
    }
    let mut found = Vec::new();
    let mut at = 2;
    loop {
        // Fill bytes may come before a marker.
        while data.get(at) == Some(&0xff) && data.get(at + 1) == Some(&0xff) {
            at += 1;
        }
        if *data.get(at)? != 0xff {
            return None;
        }
        let marker = *data.get(at + 1)?;
        if marker == SOS {
            return Some(found);
        }
        let length = usize::from(u16::from_be_bytes([*data.get(at + 2)?, *data.get(at + 3)?]));
        let payload = data.get(at + 4..at + 2 + length)?;
        found.push(Segment { marker, payload });
        at += 2 + length;
    }
}

/// The information worth carrying over from a JPEG, as whole segments ready to insert.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Carried {
    segments: Vec<Vec<u8>>,
}

impl Carried {
    /// EXIF, XMP and IPTC segments from `original`. An XMP packet describing an HDR gain map is
    /// left behind: the result is the photo's standard picture, without the gain map (item 11).
    pub fn from_jpeg(original: &[u8]) -> Self {
        let segments = segments(original)
            .unwrap_or_default()
            .into_iter()
            .filter(|s| match s.marker {
                APP1 => {
                    s.payload.starts_with(EXIF)
                        || (s.payload.starts_with(XMP) && !contains(s.payload, b"hdrgm:"))
                }
                APP13 => true,
                _ => false,
            })
            .map(|s| encode(s.marker, s.payload))
            .collect();
        Self { segments }
    }

    /// EXIF data (a TIFF structure) and an ICC profile from a picture of another format.
    pub fn from_parts(exif: Option<&[u8]>, icc: Option<&[u8]>) -> Self {
        let mut segments = Vec::new();
        if let Some(tiff) = exif {
            let payload = [EXIF, tiff].concat();
            if payload.len() + 2 <= MAX_SEGMENT {
                segments.push(encode(APP1, &payload));
            }
        }
        if let Some(icc) = icc {
            segments.extend(icc_segments(icc));
        }
        Self { segments }
    }

    pub fn is_empty(&self) -> bool {
        self.segments.is_empty()
    }

    /// `result` with the carried segments inserted after its opening JFIF segment, if it has one,
    /// or right after its start. Segments of the same kind already in `result` are replaced.
    pub fn insert_into(&self, result: &[u8]) -> Option<Vec<u8>> {
        let existing = segments(result)?;
        let carries = |marker: u8, prefix: &[u8]| {
            self.segments
                .iter()
                .any(|s| s[1] == marker && s.get(4..).is_some_and(|p| p.starts_with(prefix)))
        };
        let mut out =
            Vec::with_capacity(result.len() + self.segments.iter().map(Vec::len).sum::<usize>());
        out.extend_from_slice(&[0xff, SOI]);
        let mut inserted = false;
        let mut at = 2;
        for segment in &existing {
            let whole = 4 + segment.payload.len();
            // Find this segment's start in `result`, past any fill bytes.
            while result[at + 1] == 0xff {
                at += 1;
            }
            let bytes = &result[at..at + whole];
            at += whole;
            if !inserted && segment.marker != APP0 {
                self.segments.iter().for_each(|s| out.extend_from_slice(s));
                inserted = true;
            }
            let replaced = (segment.marker == APP1
                && (carries(APP1, EXIF) && segment.payload.starts_with(EXIF)
                    || carries(APP1, XMP) && segment.payload.starts_with(XMP)))
                || (segment.marker == APP2
                    && carries(APP2, ICC)
                    && segment.payload.starts_with(ICC))
                || (segment.marker == APP13 && carries(APP13, b""));
            if !replaced {
                out.extend_from_slice(bytes);
            }
        }
        if !inserted {
            self.segments.iter().for_each(|s| out.extend_from_slice(s));
        }
        out.extend_from_slice(&result[at..]);
        Some(out)
    }
}

/// A segment with its marker and length.
fn encode(marker: u8, payload: &[u8]) -> Vec<u8> {
    let length = (payload.len() + 2) as u16;
    let mut segment = vec![0xff, marker];
    segment.extend_from_slice(&length.to_be_bytes());
    segment.extend_from_slice(payload);
    segment
}

/// An ICC profile split across as many APP2 segments as it needs.
fn icc_segments(icc: &[u8]) -> Vec<Vec<u8>> {
    let room = MAX_SEGMENT - 2 - ICC.len() - 2;
    let chunks: Vec<&[u8]> = icc.chunks(room).collect();
    let Ok(count) = u8::try_from(chunks.len()) else {
        return Vec::new();
    };
    chunks
        .iter()
        .enumerate()
        .map(|(n, chunk)| {
            let mut payload = ICC.to_vec();
            payload.extend_from_slice(&[n as u8 + 1, count]);
            payload.extend_from_slice(chunk);
            encode(APP2, &payload)
        })
        .collect()
}

/// The EXIF data in a JPEG, as a TIFF structure.
#[cfg(test)]
pub fn exif(data: &[u8]) -> Option<&[u8]> {
    segments(data)?
        .into_iter()
        .find(|s| s.marker == APP1 && s.payload.starts_with(EXIF))
        .map(|s| &s.payload[EXIF.len()..])
}

/// Whether a JPEG carries an ICC profile.
#[cfg(test)]
pub fn has_icc(data: &[u8]) -> bool {
    segments(data)
        .unwrap_or_default()
        .iter()
        .any(|s| s.marker == APP2 && s.payload.starts_with(ICC))
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack.windows(needle.len()).any(|w| w == needle)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn jpeg(segments: &[Vec<u8>]) -> Vec<u8> {
        let mut data = vec![0xff, SOI];
        segments.iter().for_each(|s| data.extend_from_slice(s));
        data.extend_from_slice(&[0xff, SOS, 0, 2, 1, 2, 3, 0xff, 0xd9]);
        data
    }

    fn jfif() -> Vec<u8> {
        encode(APP0, b"JFIF\0\x01\x02\0\0\x01\0\x01\0\0")
    }

    #[test]
    fn copies_exif_xmp_and_iptc_after_jfif() {
        let exif = encode(APP1, b"Exif\0\0MM\0*");
        let xmp = encode(APP1, &[XMP, b"<x/>"].concat());
        let iptc = encode(APP13, b"Photoshop 3.0\0");
        let mpf = encode(APP2, b"MPF\0data");
        let original = jpeg(&[jfif(), exif.clone(), mpf, xmp.clone(), iptc.clone()]);
        let icc = encode(APP2, b"ICC_PROFILE\0\x01\x01profile");
        let result = jpeg(&[jfif(), icc.clone()]);

        let carried = Carried::from_jpeg(&original);
        let out = carried.insert_into(&result).unwrap();
        assert_eq!(out, jpeg(&[jfif(), exif, xmp, iptc, icc]));
        assert_eq!(super::exif(&out), Some(&b"MM\0*"[..]));
        assert!(has_icc(&out));
    }

    #[test]
    fn leaves_a_gain_map_description_behind() {
        let xmp = encode(APP1, &[XMP, b"<hdrgm:Version/>"].concat());
        let carried = Carried::from_jpeg(&jpeg(&[xmp]));
        assert!(carried.is_empty());
    }

    #[test]
    fn inserts_right_after_the_start_without_jfif() {
        let carried = Carried::from_parts(Some(b"II*\0"), None);
        let dqt = encode(0xdb, &[0; 65]);
        let out = carried
            .insert_into(&jpeg(std::slice::from_ref(&dqt)))
            .unwrap();
        assert_eq!(out, jpeg(&[encode(APP1, b"Exif\0\0II*\0"), dqt]));
    }

    #[test]
    fn a_large_profile_is_split() {
        let icc = vec![7u8; 150_000];
        let segments = icc_segments(&icc);
        assert_eq!(segments.len(), 3);
        assert!(segments.iter().all(|s| s.len() <= MAX_SEGMENT + 2));
        let rejoined: Vec<u8> = segments.iter().flat_map(|s| s[18..].to_vec()).collect();
        assert_eq!(rejoined, icc);
        assert_eq!(&segments[2][16..18], &[3, 3]);
    }

    #[test]
    fn not_a_jpeg_is_refused() {
        assert_eq!(Carried::default().insert_into(b"PNG"), None);
        assert_eq!(exif(b""), None);
    }
}
