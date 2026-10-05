//! Reading what a HEIC, HEIF or AVIF photo says about itself: its EXIF data, its color profile,
//! whether its main picture is stored as true HDR and whether it has transparency.
//!
//! FFmpeg decodes these pictures but leaves the EXIF data and the color profile behind, so Gracil
//! reads them from the file's boxes itself (ISO/IEC 23008-12): the primary item (`pitm`), its
//! properties (`ipco`, `ipma`), the EXIF item that describes it (`iinf`, `iref` `cdsc`) and where
//! that item's bytes are (`iloc`). Anything unexpected simply leaves a field empty.

/// What was found. Every field is optional: a photo without any of it is still compressed.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct HeifInfo {
    /// The EXIF data as a TIFF structure, without the `Exif\0\0` prefix.
    pub exif: Option<Vec<u8>>,
    /// The ICC color profile of the primary picture.
    pub icc: Option<Vec<u8>>,
    /// The primary picture is stored as PQ or HLG (true HDR).
    pub hdr: bool,
    /// The primary picture's transparency (alpha), kept as a picture of its own.
    pub alpha: Option<Alpha>,
}

/// The item holding the primary picture's transparency.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Alpha {
    pub item: u32,
    /// Laid out as a grid of tiles, which FFmpeg reads only one tile at a time.
    pub grid: bool,
}

pub fn read(data: &[u8]) -> HeifInfo {
    let Some(meta) = boxes(data).find(|b| &b.kind == b"meta") else {
        return HeifInfo::default();
    };
    // `meta` is a full box: version and flags first.
    let meta = meta.body.get(4..).unwrap_or_default();
    let children = || boxes(meta);

    let primary = children()
        .find(|b| &b.kind == b"pitm")
        .and_then(|b| read_pitm(b.body));
    let items = children()
        .find(|b| &b.kind == b"iinf")
        .map(|b| read_iinf(b.body))
        .unwrap_or_default();
    let references = children()
        .find(|b| &b.kind == b"iref")
        .map(|b| read_references(b.body, b"cdsc"))
        .unwrap_or_default();
    let locations = children()
        .find(|b| &b.kind == b"iloc")
        .map(|b| read_iloc(b.body))
        .unwrap_or_default();
    let idat = children().find(|b| &b.kind == b"idat").map(|b| b.body);
    let (properties, associations) = children()
        .find(|b| &b.kind == b"iprp")
        .map(|b| read_iprp(b.body))
        .unwrap_or_default();

    // Transparency is an auxiliary item (`auxl`) of the primary one, described as alpha. Other
    // auxiliary items, such as an iPhone's depth and portrait mattes, are not transparency.
    let auxiliary = children()
        .find(|b| &b.kind == b"iref")
        .map(|b| read_references(b.body, b"auxl"))
        .unwrap_or_default();
    let is_alpha = |item: u32| {
        associations
            .iter()
            .filter(|(id, _)| *id == item)
            .flat_map(|(_, indices)| indices)
            .filter_map(|&index| index.checked_sub(1).and_then(|i| properties.get(i)))
            .any(|p| {
                let urn = p.body.get(4..).unwrap_or_default();
                &p.kind == b"auxC"
                    && (urn.starts_with(b"urn:mpeg:mpegB:cicp:systems:auxiliary:alpha")
                        || urn.starts_with(b"urn:mpeg:hevc:2015:auxid:1\0"))
            })
    };
    let mut info = HeifInfo {
        alpha: auxiliary
            .iter()
            .find(|(from, to)| Some(*to) == primary && is_alpha(*from))
            .map(|&(item, _)| Alpha {
                item,
                grid: items
                    .iter()
                    .any(|(id, kind)| *id == item && kind == b"grid"),
            }),
        ..HeifInfo::default()
    };

    if let Some(primary) = primary {
        for &index in associations
            .iter()
            .filter(|(item, _)| *item == primary)
            .flat_map(|(_, indices)| indices)
        {
            let Some(property) = index.checked_sub(1).and_then(|i| properties.get(i)) else {
                continue;
            };
            if &property.kind != b"colr" || property.body.len() < 4 {
                continue;
            }
            match &property.body[..4] {
                b"prof" | b"rICC" => info.icc = Some(property.body[4..].to_vec()),
                b"nclx" => {
                    let transfer = be16(property.body, 6);
                    info.hdr |= matches!(transfer, Some(16 | 18));
                }
                _ => {}
            }
        }
    }

    // The EXIF item describing the primary picture, or the only one there is.
    let exif_items: Vec<u32> = items
        .iter()
        .filter(|(_, kind)| kind == b"Exif")
        .map(|(id, _)| *id)
        .collect();
    let exif_item = exif_items
        .iter()
        .copied()
        .find(|id| {
            references
                .iter()
                .any(|(from, to)| from == id && Some(*to) == primary)
        })
        .or(match exif_items.as_slice() {
            [only] => Some(*only),
            _ => None,
        });
    info.exif = exif_item
        .and_then(|id| locations.iter().find(|l| l.item == id))
        .and_then(|location| location.bytes(data, idat))
        .and_then(|payload| {
            // A 4-byte offset to the TIFF header, then the data (often starting with "Exif\0\0").
            let offset = be32(&payload, 0)? as usize;
            let tiff = payload.get(4 + offset..)?;
            Some(super::exif::strip_prefix(tiff).to_vec())
        });
    info
}

struct Atom<'a> {
    kind: [u8; 4],
    body: &'a [u8],
}

/// The boxes laid one after another in `data`.
fn boxes(mut data: &[u8]) -> impl Iterator<Item = Atom<'_>> {
    std::iter::from_fn(move || {
        let size = be32(data, 0)? as u64;
        let kind: [u8; 4] = data.get(4..8)?.try_into().ok()?;
        let (header, size) = match size {
            0 => (8, data.len() as u64),
            1 => (16, be64(data, 8)?),
            n => (8, n),
        };
        let size = usize::try_from(size).ok()?;
        if size < header || size > data.len() {
            return None;
        }
        let body = &data[header..size];
        data = &data[size..];
        Some(Atom { kind, body })
    })
}

fn read_pitm(body: &[u8]) -> Option<u32> {
    match body.first()? {
        0 => be16(body, 4).map(u32::from),
        _ => be32(body, 4),
    }
}

/// Item ids and their types, from `iinf`'s `infe` boxes (version 2 or later).
fn read_iinf(body: &[u8]) -> Vec<(u32, [u8; 4])> {
    let start = if body.first() == Some(&0) { 6 } else { 8 };
    boxes(body.get(start..).unwrap_or_default())
        .filter(|b| &b.kind == b"infe")
        .filter_map(|infe| {
            let body = infe.body;
            match body.first()? {
                2 => Some((u32::from(be16(body, 4)?), body.get(8..12)?.try_into().ok()?)),
                3 => Some((be32(body, 4)?, body.get(10..14)?.try_into().ok()?)),
                _ => None,
            }
        })
        .collect()
}

/// `(from, to)` pairs of the item references of type `kind`: `cdsc` for metadata items and what
/// they describe, `auxl` for auxiliary pictures and the picture they belong to.
fn read_references(body: &[u8], kind: &[u8; 4]) -> Vec<(u32, u32)> {
    let wide = body.first().is_some_and(|&version| version != 0);
    let id = |data: &[u8], at: usize| {
        if wide {
            be32(data, at)
        } else {
            be16(data, at).map(u32::from)
        }
    };
    let width = if wide { 4 } else { 2 };
    let mut pairs = Vec::new();
    for reference in boxes(body.get(4..).unwrap_or_default()) {
        if &reference.kind != kind {
            continue;
        }
        let data = reference.body;
        let (Some(from), Some(count)) = (id(data, 0), be16(data, width)) else {
            continue;
        };
        for n in 0..usize::from(count) {
            if let Some(to) = id(data, width + 2 + n * width) {
                pairs.push((from, to));
            }
        }
    }
    pairs
}

struct Location {
    item: u32,
    /// 0: in the file; 1: in the `idat` box.
    construction: u8,
    extents: Vec<(u64, u64)>,
}

impl Location {
    fn bytes(&self, file: &[u8], idat: Option<&[u8]>) -> Option<Vec<u8>> {
        let source = match self.construction {
            0 => file,
            1 => idat?,
            _ => return None,
        };
        let mut bytes = Vec::new();
        for &(offset, length) in &self.extents {
            let start = usize::try_from(offset).ok()?;
            let end = start.checked_add(usize::try_from(length).ok()?)?;
            bytes.extend_from_slice(source.get(start..end)?);
        }
        Some(bytes)
    }
}

fn read_iloc(body: &[u8]) -> Vec<Location> {
    read_iloc_checked(body).unwrap_or_default()
}

fn read_iloc_checked(body: &[u8]) -> Option<Vec<Location>> {
    let version = *body.first()?;
    let sizes = be16(body, 4)?;
    let offset_size = usize::from(sizes >> 12);
    let length_size = usize::from((sizes >> 8) & 0xf);
    let base_size = usize::from((sizes >> 4) & 0xf);
    let index_size = if version >= 1 {
        usize::from(sizes & 0xf)
    } else {
        0
    };
    let mut at = 6;
    let count = if version < 2 {
        at += 2;
        u32::from(be16(body, 6)?)
    } else {
        at += 4;
        be32(body, 6)?
    };
    let mut locations = Vec::new();
    for _ in 0..count {
        let item = if version < 2 {
            at += 2;
            u32::from(be16(body, at - 2)?)
        } else {
            at += 4;
            be32(body, at - 4)?
        };
        let construction = if version >= 1 {
            at += 2;
            (be16(body, at - 2)? & 0xf) as u8
        } else {
            0
        };
        at += 2; // data reference index
        let base = sized(body, at, base_size)?;
        at += base_size;
        let extent_count = be16(body, at)?;
        at += 2;
        let mut extents = Vec::new();
        for _ in 0..extent_count {
            at += index_size;
            let offset = sized(body, at, offset_size)?;
            at += offset_size;
            let length = sized(body, at, length_size)?;
            at += length_size;
            extents.push((base + offset, length));
        }
        locations.push(Location {
            item,
            construction,
            extents,
        });
    }
    Some(locations)
}

struct Property<'a> {
    kind: [u8; 4],
    body: &'a [u8],
}

/// The item properties in order, and which of them (1-based) belong to each item.
fn read_iprp(body: &[u8]) -> (Vec<Property<'_>>, Vec<(u32, Vec<usize>)>) {
    let mut properties = Vec::new();
    let mut associations = Vec::new();
    for child in boxes(body) {
        match &child.kind {
            b"ipco" => {
                properties = boxes(child.body)
                    .map(|b| Property {
                        kind: b.kind,
                        body: b.body,
                    })
                    .collect();
            }
            b"ipma" => associations.extend(read_ipma(child.body).unwrap_or_default()),
            _ => {}
        }
    }
    (properties, associations)
}

fn read_ipma(body: &[u8]) -> Option<Vec<(u32, Vec<usize>)>> {
    let version = *body.first()?;
    let wide_index = body.get(3)? & 1 == 1;
    let count = be32(body, 4)?;
    let mut at = 8;
    let mut associations = Vec::new();
    for _ in 0..count {
        let item = if version < 1 {
            at += 2;
            u32::from(be16(body, at - 2)?)
        } else {
            at += 4;
            be32(body, at - 4)?
        };
        let n = *body.get(at)?;
        at += 1;
        let mut indices = Vec::new();
        for _ in 0..n {
            let index = if wide_index {
                at += 2;
                usize::from(be16(body, at - 2)? & 0x7fff)
            } else {
                at += 1;
                usize::from(body.get(at - 1)? & 0x7f)
            };
            indices.push(index);
        }
        associations.push((item, indices));
    }
    Some(associations)
}

fn sized(data: &[u8], at: usize, size: usize) -> Option<u64> {
    match size {
        0 => Some(0),
        4 => be32(data, at).map(u64::from),
        8 => be64(data, at),
        _ => None,
    }
}

fn be16(data: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_be_bytes(data.get(at..at + 2)?.try_into().ok()?))
}

fn be32(data: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_be_bytes(data.get(at..at + 4)?.try_into().ok()?))
}

fn be64(data: &[u8], at: usize) -> Option<u64> {
    Some(u64::from_be_bytes(data.get(at..at + 8)?.try_into().ok()?))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn boxed(kind: &[u8; 4], body: &[u8]) -> Vec<u8> {
        let mut out = ((body.len() + 8) as u32).to_be_bytes().to_vec();
        out.extend_from_slice(kind);
        out.extend_from_slice(body);
        out
    }

    fn full(kind: &[u8; 4], version: u8, body: &[u8]) -> Vec<u8> {
        let mut with_header = vec![version, 0, 0, 0];
        with_header.extend_from_slice(body);
        boxed(kind, &with_header)
    }

    /// A minimal HEIF file: item 1 is the picture, item 2 its EXIF data, kept in `idat`.
    fn sample(transfer: u16) -> Vec<u8> {
        sample_with(transfer, None)
    }

    /// [`sample`], and with `aux`, item 3: an auxiliary picture of item 1 of that type.
    fn sample_with(transfer: u16, aux: Option<&[u8]>) -> Vec<u8> {
        let tiff = b"MM\0\x2a\0\0\0\x08\0\0";
        let mut exif = 6u32.to_be_bytes().to_vec();
        exif.extend_from_slice(b"Exif\0\0");
        exif.extend_from_slice(tiff);

        let pitm = full(b"pitm", 0, &1u16.to_be_bytes());
        let infe = |id: u16, kind: &[u8; 4]| {
            let mut body = id.to_be_bytes().to_vec();
            body.extend_from_slice(&[0, 0]);
            body.extend_from_slice(kind);
            body.push(0);
            full(b"infe", 2, &body)
        };
        let count: u16 = if aux.is_some() { 3 } else { 2 };
        let mut iinf_body = count.to_be_bytes().to_vec();
        iinf_body.extend(infe(1, b"hvc1"));
        iinf_body.extend(infe(2, b"Exif"));
        if aux.is_some() {
            iinf_body.extend(infe(3, b"hvc1"));
        }
        let iinf = full(b"iinf", 0, &iinf_body);

        let mut cdsc = 2u16.to_be_bytes().to_vec();
        cdsc.extend_from_slice(&1u16.to_be_bytes());
        cdsc.extend_from_slice(&1u16.to_be_bytes());
        let mut references = boxed(b"cdsc", &cdsc);
        if aux.is_some() {
            let mut auxl = 3u16.to_be_bytes().to_vec();
            auxl.extend_from_slice(&1u16.to_be_bytes());
            auxl.extend_from_slice(&1u16.to_be_bytes());
            references.extend(boxed(b"auxl", &auxl));
        }
        let iref = full(b"iref", 0, &references);

        // version 1, offset 4 bytes, length 4 bytes, base 0, index 0; one item in idat.
        let mut iloc_body = vec![0x44, 0x00];
        iloc_body.extend_from_slice(&1u16.to_be_bytes());
        iloc_body.extend_from_slice(&2u16.to_be_bytes());
        iloc_body.extend_from_slice(&1u16.to_be_bytes()); // construction method 1
        iloc_body.extend_from_slice(&0u16.to_be_bytes());
        iloc_body.extend_from_slice(&1u16.to_be_bytes());
        iloc_body.extend_from_slice(&0u32.to_be_bytes());
        iloc_body.extend_from_slice(&(exif.len() as u32).to_be_bytes());
        let iloc = full(b"iloc", 1, &iloc_body);
        let idat = boxed(b"idat", &exif);

        let mut prof = b"prof".to_vec();
        prof.extend_from_slice(b"an icc profile");
        let mut nclx = b"nclx".to_vec();
        nclx.extend_from_slice(&1u16.to_be_bytes());
        nclx.extend_from_slice(&transfer.to_be_bytes());
        nclx.extend_from_slice(&[0, 1, 0x80]);
        let mut ipco = boxed(b"colr", &prof);
        ipco.extend(boxed(b"colr", &nclx));
        let entries: u32 = if aux.is_some() { 2 } else { 1 };
        let mut ipma_body = entries.to_be_bytes().to_vec();
        ipma_body.extend_from_slice(&1u16.to_be_bytes());
        ipma_body.extend_from_slice(&[2, 0x81, 0x02]);
        if let Some(urn) = aux {
            let mut auxc = urn.to_vec();
            auxc.push(0);
            ipco.extend(full(b"auxC", 0, &auxc));
            ipma_body.extend_from_slice(&3u16.to_be_bytes());
            ipma_body.extend_from_slice(&[1, 0x83]);
        }
        let mut iprp = boxed(b"ipco", &ipco);
        iprp.extend(full(b"ipma", 0, &ipma_body));
        let iprp = boxed(b"iprp", &iprp);

        let mut meta = Vec::new();
        for part in [pitm, iinf, iref, iloc, idat, iprp] {
            meta.extend(part);
        }
        let mut file = boxed(b"ftyp", b"heic\0\0\0\0mif1heic");
        file.extend(full(b"meta", 0, &meta));
        file
    }

    #[test]
    fn finds_the_primary_pictures_exif_and_profile() {
        let info = read(&sample(1));
        assert_eq!(info.exif.as_deref(), Some(&b"MM\0\x2a\0\0\0\x08\0\0"[..]));
        assert_eq!(info.icc.as_deref(), Some(&b"an icc profile"[..]));
        assert!(!info.hdr);
        assert_eq!(info.alpha, None);
    }

    #[test]
    fn finds_transparency_but_not_an_iphones_mattes() {
        for urn in [
            &b"urn:mpeg:hevc:2015:auxid:1"[..],
            b"urn:mpeg:mpegB:cicp:systems:auxiliary:alpha",
        ] {
            let alpha = read(&sample_with(1, Some(urn))).alpha;
            assert_eq!(
                alpha,
                Some(Alpha {
                    item: 3,
                    grid: false
                }),
                "{urn:?}"
            );
        }
        for urn in [
            &b"urn:mpeg:hevc:2015:auxid:2"[..],
            b"urn:com:apple:photo:2018:aux:portraiteffectsmatte",
            b"urn:com:apple:photo:2020:aux:hdrgainmap",
        ] {
            assert_eq!(read(&sample_with(1, Some(urn))).alpha, None, "{urn:?}");
        }
    }

    #[test]
    fn notices_true_hdr() {
        assert!(read(&sample(16)).hdr);
        assert!(read(&sample(18)).hdr);
    }

    #[test]
    fn anything_else_finds_nothing() {
        assert_eq!(read(b"not a heif file at all"), HeifInfo::default());
        assert_eq!(read(&[]), HeifInfo::default());
    }
}
