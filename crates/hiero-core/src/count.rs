//! How many of something run, such as a server's instances: a whole number, a minimum such as
//! "50+", or a range such as "50-100". A number is stored as a JSON number, as the prototype
//! stores it; a minimum or a range as text in one spelling.

use std::fmt;

use serde::de::{self, Deserializer, Visitor};
use serde::{Deserialize, Serialize, Serializer};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstanceCount {
    Exactly(u32),
    /// This many or more, written "50+".
    AtLeast(u32),
    /// From the first to the second, written "50-100". The first is the smaller.
    Between(u32, u32),
}

impl InstanceCount {
    /// What to say when a count isn't one of the forms, or is out of bounds.
    pub fn hint(max: u32) -> String {
        format!("Enter a whole number from 1 to {max}, or a range such as 50+ or 50-100.")
    }

    /// Reads a count as typed, allowing spaces and an en or em dash. Numbers go from 1 to `max`,
    /// and a range's first number is the smaller.
    pub fn parse(text: &str, max: u32) -> Result<Self, String> {
        let text: String = text
            .chars()
            .filter(|c| !c.is_whitespace())
            .map(|c| if matches!(c, '–' | '—') { '-' } else { c })
            .collect();
        let number = |s: &str| {
            s.parse::<u32>()
                .ok()
                .filter(|n| (1..=max).contains(n) && s.chars().all(|c| c.is_ascii_digit()))
        };
        let count = if let Some(low) = text.strip_suffix('+') {
            number(low).map(Self::AtLeast)
        } else if let Some((low, high)) = text.split_once('-') {
            match (number(low), number(high)) {
                (Some(low), Some(high)) if low < high => Some(Self::Between(low, high)),
                _ => None,
            }
        } else {
            number(&text).map(Self::Exactly)
        };
        count.ok_or_else(|| Self::hint(max))
    }

    /// Whether the numbers are from 1 to `max`, with a range's first the smaller.
    pub fn is_within(self, max: u32) -> bool {
        let ok = |n: u32| (1..=max).contains(&n);
        match self {
            Self::Exactly(n) | Self::AtLeast(n) => ok(n),
            Self::Between(low, high) => ok(low) && ok(high) && low < high,
        }
    }

    /// Whether it's just one, which is left out when stored.
    pub fn is_one(self) -> bool {
        self == Self::Exactly(1)
    }

    /// Such as "×3", "×50+", or "×50–100", drawn beside a figure.
    pub fn badge(self) -> String {
        match self {
            Self::Exactly(n) => format!("×{n}"),
            Self::AtLeast(n) => format!("×{n}+"),
            Self::Between(low, high) => format!("×{low}–{high}"),
        }
    }

    /// Such as "3 instances", "50 or more instances", or "50 to 100 instances".
    pub fn describe(self, noun: &str) -> String {
        let plural = |n: u32| if n == 1 { "" } else { "s" };
        match self {
            Self::Exactly(n) => format!("{n} {noun}{}", plural(n)),
            Self::AtLeast(n) => format!("{n} or more {noun}s"),
            Self::Between(low, high) => format!("{low} to {high} {noun}s"),
        }
    }
}

/// As typed and stored: "3", "50+", or "50-100".
impl fmt::Display for InstanceCount {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Exactly(n) => write!(f, "{n}"),
            Self::AtLeast(n) => write!(f, "{n}+"),
            Self::Between(low, high) => write!(f, "{low}-{high}"),
        }
    }
}

impl Serialize for InstanceCount {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Exactly(n) => s.serialize_u32(*n),
            _ => s.serialize_str(&self.to_string()),
        }
    }
}

impl<'de> Deserialize<'de> for InstanceCount {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct CountVisitor;

        impl Visitor<'_> for CountVisitor {
            type Value = InstanceCount;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("a whole number, or a range such as \"50+\" or \"50-100\"")
            }

            fn visit_u64<E: de::Error>(self, n: u64) -> Result<Self::Value, E> {
                u32::try_from(n)
                    .map(InstanceCount::Exactly)
                    .map_err(|_| E::custom("the number is too big"))
            }

            fn visit_i64<E: de::Error>(self, n: i64) -> Result<Self::Value, E> {
                u64::try_from(n)
                    .map_err(|_| E::custom("the number is below 0"))
                    .and_then(|n| self.visit_u64(n))
            }

            fn visit_f64<E: de::Error>(self, n: f64) -> Result<Self::Value, E> {
                if n.fract() == 0.0 && (0.0..=f64::from(u32::MAX)).contains(&n) {
                    Ok(InstanceCount::Exactly(n as u32))
                } else {
                    Err(E::custom("the number isn't whole"))
                }
            }

            fn visit_str<E: de::Error>(self, text: &str) -> Result<Self::Value, E> {
                InstanceCount::parse(text, u32::MAX).map_err(E::custom)
            }
        }

        d.deserialize_any(CountVisitor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn counts_read_as_typed() {
        let parse = |t| InstanceCount::parse(t, 9999);
        assert_eq!(parse("3"), Ok(InstanceCount::Exactly(3)));
        assert_eq!(parse(" 50+ "), Ok(InstanceCount::AtLeast(50)));
        assert_eq!(parse("50 – 100"), Ok(InstanceCount::Between(50, 100)));
        for bad in [
            "", "0", "10000", "100-50", "5-5", "-5", "+5", "lots", "1.5", "50++",
        ] {
            assert_eq!(parse(bad), Err(InstanceCount::hint(9999)), "{bad}");
        }
    }

    #[test]
    fn numbers_stay_numbers_and_ranges_are_text() {
        let json = |c: InstanceCount| serde_json::to_value(c).unwrap();
        assert_eq!(json(InstanceCount::Exactly(3)), json!(3));
        assert_eq!(json(InstanceCount::AtLeast(50)), json!("50+"));
        assert_eq!(json(InstanceCount::Between(50, 100)), json!("50-100"));
        let read = |v| serde_json::from_value::<InstanceCount>(v).unwrap();
        assert_eq!(read(json!(3)), InstanceCount::Exactly(3));
        assert_eq!(read(json!("50-100")), InstanceCount::Between(50, 100));
    }

    #[test]
    fn counts_are_drawn_and_read_out() {
        assert_eq!(InstanceCount::Between(50, 100).badge(), "×50–100");
        assert_eq!(
            InstanceCount::AtLeast(50).describe("instance"),
            "50 or more instances"
        );
        assert_eq!(
            InstanceCount::Between(2, 4).describe("node"),
            "2 to 4 nodes"
        );
    }
}
