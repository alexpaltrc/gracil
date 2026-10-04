//! The one EXIF field Gracil changes: the orientation.
//!
//! EXIF data is carried over as it is (ADR 0034, item 10), except when Gracil has already turned
//! the picture upright itself (formats other than JPEG); then the orientation must say "upright",
//! or viewers would turn it a second time.

const ORIENTATION: u16 = 0x0112;
const SHORT: u16 = 3;

/// `data` without a leading `Exif\0\0`, which some files put before the TIFF structure.
pub fn strip_prefix(data: &[u8]) -> &[u8] {
    data.strip_prefix(b"Exif\0\0").unwrap_or(data)
}

/// The orientation recorded in a TIFF structure, 1 to 8, if there is one.
#[cfg(test)]
pub fn orientation(tiff: &[u8]) -> Option<u16> {
    let (at, order) = orientation_value(tiff)?;
    order.u16(tiff, at)
}

/// Records "upright" (1) as the orientation, if the data has one. Returns whether it changed.
pub fn set_upright(tiff: &mut [u8]) -> bool {
    let Some((at, order)) = orientation_value(tiff) else {
        return false;
    };
    let upright = match order {
        Order::Little => 1u16.to_le_bytes(),
        Order::Big => 1u16.to_be_bytes(),
    };
    if tiff[at..at + 2] == upright {
        return false;
    }
    tiff[at..at + 2].copy_from_slice(&upright);
    true
}

#[derive(Clone, Copy)]
enum Order {
    Little,
    Big,
}

impl Order {
    fn u16(self, data: &[u8], at: usize) -> Option<u16> {
        let bytes = data.get(at..at + 2)?.try_into().ok()?;
        Some(match self {
            Self::Little => u16::from_le_bytes(bytes),
            Self::Big => u16::from_be_bytes(bytes),
        })
    }

    fn u32(self, data: &[u8], at: usize) -> Option<u32> {
        let bytes = data.get(at..at + 4)?.try_into().ok()?;
        Some(match self {
            Self::Little => u32::from_le_bytes(bytes),
            Self::Big => u32::from_be_bytes(bytes),
        })
    }
}

/// Where the orientation's value sits in the first directory (IFD0), and the byte order.
fn orientation_value(tiff: &[u8]) -> Option<(usize, Order)> {
    let order = match tiff.get(..4)? {
        b"II*\0" => Order::Little,
        b"MM\0*" => Order::Big,
        _ => return None,
    };
    let ifd = order.u32(tiff, 4)? as usize;
    let count = usize::from(order.u16(tiff, ifd)?);
    (0..count).find_map(|n| {
        let entry = ifd + 2 + n * 12;
        let is_orientation = order.u16(tiff, entry)? == ORIENTATION
            && order.u16(tiff, entry + 2)? == SHORT
            && order.u32(tiff, entry + 4)? == 1;
        // A single short sits at the start of the entry's 4-byte value field.
        (is_orientation && tiff.len() >= entry + 10).then_some((entry + 8, order))
    })
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A TIFF structure with one entry: the orientation.
    pub(crate) fn with_orientation(value: u16, little: bool) -> Vec<u8> {
        let mut tiff = Vec::new();
        let u16_ = |v: u16| {
            if little {
                v.to_le_bytes()
            } else {
                v.to_be_bytes()
            }
        };
        let u32_ = |v: u32| {
            if little {
                v.to_le_bytes()
            } else {
                v.to_be_bytes()
            }
        };
        tiff.extend_from_slice(if little { b"II*\0" } else { b"MM\0*" });
        tiff.extend_from_slice(&u32_(8));
        tiff.extend_from_slice(&u16_(1));
        tiff.extend_from_slice(&u16_(ORIENTATION));
        tiff.extend_from_slice(&u16_(SHORT));
        tiff.extend_from_slice(&u32_(1));
        tiff.extend_from_slice(&u16_(value));
        tiff.extend_from_slice(&[0, 0]);
        tiff.extend_from_slice(&u32_(0));
        tiff
    }

    #[test]
    fn turns_the_orientation_upright_in_either_byte_order() {
        for little in [true, false] {
            let mut tiff = with_orientation(6, little);
            assert_eq!(orientation(&tiff), Some(6));
            assert!(set_upright(&mut tiff));
            assert_eq!(orientation(&tiff), Some(1));
            assert!(!set_upright(&mut tiff));
        }
    }

    #[test]
    fn leaves_data_without_an_orientation_alone() {
        let mut nothing = b"MM\0*\0\0\0\x08\0\0".to_vec();
        assert!(!set_upright(&mut nothing));
        assert!(!set_upright(&mut [1, 2, 3]));
        assert_eq!(orientation(b"garbage"), None);
    }

    #[test]
    fn the_exif_prefix_is_optional() {
        assert_eq!(strip_prefix(b"Exif\0\0MM"), b"MM");
        assert_eq!(strip_prefix(b"MM"), b"MM");
    }
}
