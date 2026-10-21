//! How dates are shown.

/// An RFC 3339 timestamp in local time, such as "20 Oct 2026, 17:42".
pub fn date(rfc3339: &str) -> String {
    rfc3339.parse::<jiff::Timestamp>().map_or_else(
        |_| rfc3339.to_owned(),
        |t| {
            t.to_zoned(jiff::tz::TimeZone::system())
                .strftime("%-d %b %Y, %H:%M")
                .to_string()
        },
    )
}

#[cfg(test)]
mod tests {
    #[test]
    fn unreadable_dates_are_shown_as_they_are() {
        assert_eq!(super::date("soon"), "soon");
        assert!(super::date("2026-10-01T09:00:00.000Z").contains("2026"));
    }
}
