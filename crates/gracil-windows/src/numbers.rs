//! How the user has Windows write numbers (spec §77.3).

use std::ptr;

use windows_sys::Win32::Globalization::{
    GetLocaleInfoEx, LOCALE_SDECIMAL, LOCALE_SGROUPING, LOCALE_STHOUSAND,
};

/// The number settings of the user's region, as Windows stores them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NumberSettings {
    /// The decimal mark, such as `.` or `,`.
    pub decimal: String,
    /// What separates groups of digits, such as `,`, `.` or a space.
    pub thousands: String,
    /// The size of each group from the right, such as `3;0` (Windows' own notation).
    pub grouping: String,
}

/// The user's number settings, or `None` if Windows can't say.
pub fn number_settings() -> Option<NumberSettings> {
    Some(NumberSettings {
        decimal: locale_info(LOCALE_SDECIMAL)?,
        thousands: locale_info(LOCALE_STHOUSAND)?,
        grouping: locale_info(LOCALE_SGROUPING)?,
    })
}

fn locale_info(kind: u32) -> Option<String> {
    // Windows allows at most four characters for these, plus the terminating NUL.
    let mut buffer = [0u16; 16];
    // SAFETY: a null locale name means the user's default locale; the buffer is writable for the
    // length passed, and Windows writes at most that many characters.
    #[allow(unsafe_code)]
    let written =
        unsafe { GetLocaleInfoEx(ptr::null(), kind, buffer.as_mut_ptr(), buffer.len() as i32) };
    // The count includes the terminating NUL.
    let length = usize::try_from(written).ok()?.checked_sub(1)?;
    String::from_utf16(&buffer[..length]).ok()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::*;

    #[test]
    fn windows_knows_how_numbers_are_written() {
        let settings = number_settings().expect("every Windows user has number settings");
        assert!(!settings.decimal.is_empty());
        assert!(settings.grouping.starts_with(|c: char| c.is_ascii_digit()));
    }
}
