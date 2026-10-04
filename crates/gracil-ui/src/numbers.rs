//! Numbers written the way the user has Windows write them: `15,031`, `15.031` or `15 031`, and
//! `1.2` or `1,2` (spec §77.3).

use std::sync::OnceLock;

use gracil_windows::NumberSettings;

/// How to write a number: its decimal mark and how its digits are grouped.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Numbers {
    decimal: String,
    thousands: String,
    /// The size of each group of digits, from the right.
    groups: Vec<usize>,
    /// Whether the last group size repeats for the digits further left.
    repeat: bool,
}

impl Numbers {
    /// The user's own format, read from Windows once. Tests always use [`Self::plain`], so they
    /// don't depend on the machine's region.
    pub fn system() -> &'static Self {
        static SYSTEM: OnceLock<Numbers> = OnceLock::new();
        SYSTEM.get_or_init(|| {
            if cfg!(test) {
                return Self::plain();
            }
            gracil_windows::number_settings()
                .map_or_else(Self::plain, |settings| Self::from_settings(&settings))
        })
    }

    /// `15,031` and `1.2`, when Windows can't say.
    pub fn plain() -> Self {
        Self::from_settings(&NumberSettings {
            decimal: ".".to_owned(),
            thousands: ",".to_owned(),
            grouping: "3;0".to_owned(),
        })
    }

    /// Reads Windows' notation for groups: `3;0` is groups of three, `3;2;0` is three then twos
    /// (as in India), and a list without the final `0` leaves the remaining digits together.
    pub fn from_settings(settings: &NumberSettings) -> Self {
        let mut groups: Vec<usize> = settings
            .grouping
            .split(';')
            .filter_map(|size| size.trim().parse().ok())
            .collect();
        let repeat = groups.last() == Some(&0);
        groups.retain(|&size| size > 0);
        Self {
            decimal: settings.decimal.clone(),
            thousands: settings.thousands.clone(),
            repeat: repeat && !groups.is_empty(),
            groups,
        }
    }

    /// A whole number: `15,031`.
    pub fn whole(&self, number: u64) -> String {
        self.grouped(&number.to_string())
    }

    /// A number with one decimal: `1.2`.
    pub fn one_decimal(&self, number: f64) -> String {
        let written = format!("{number:.1}");
        match written.split_once('.') {
            Some((whole, fraction)) => format!("{}{}{fraction}", self.grouped(whole), self.decimal),
            None => self.grouped(&written),
        }
    }

    fn grouped(&self, digits: &str) -> String {
        let mut parts = Vec::new();
        let mut rest = digits;
        let mut sizes = self.groups.iter().copied();
        let mut size = None;
        loop {
            size = sizes.next().or(if self.repeat { size } else { None });
            match size {
                Some(size) if rest.len() > size => {
                    let (left, right) = rest.split_at(rest.len() - size);
                    parts.push(right);
                    rest = left;
                }
                _ => break,
            }
        }
        parts.push(rest);
        parts.reverse();
        parts.join(&self.thousands)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn numbers(decimal: &str, thousands: &str, grouping: &str) -> Numbers {
        Numbers::from_settings(&NumberSettings {
            decimal: decimal.to_owned(),
            thousands: thousands.to_owned(),
            grouping: grouping.to_owned(),
        })
    }

    #[test]
    fn groups_of_three() {
        let plain = Numbers::plain();
        assert_eq!(plain.whole(7), "7");
        assert_eq!(plain.whole(999), "999");
        assert_eq!(plain.whole(15_031), "15,031");
        assert_eq!(plain.whole(1_234_567), "1,234,567");
        assert_eq!(plain.one_decimal(1.24), "1.2");
        assert_eq!(plain.one_decimal(1234.5), "1,234.5");
    }

    #[test]
    fn follows_the_region() {
        let spain = numbers(",", ".", "3;0");
        assert_eq!(spain.whole(15_031), "15.031");
        assert_eq!(spain.one_decimal(1.2), "1,2");
        let france = numbers(",", "\u{202f}", "3;0");
        assert_eq!(france.whole(15_031), "15\u{202f}031");
        let india = numbers(".", ",", "3;2;0");
        assert_eq!(india.whole(123_456_789), "12,34,56,789");
    }

    #[test]
    fn a_grouping_that_does_not_repeat() {
        assert_eq!(numbers(".", ",", "3").whole(123_456_789), "123456,789");
        assert_eq!(numbers(".", ",", "3;2").whole(123_456_789), "1234,56,789");
        assert_eq!(numbers(".", ",", "0").whole(123_456), "123456");
    }
}
