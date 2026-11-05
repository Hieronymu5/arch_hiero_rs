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

/// The app's version and when it was built, in local time, such as
/// "Version 0.1.0 · Built 4 Nov 2026, 19:20".
pub fn version_and_build() -> String {
    format!(
        "Version {} · Built {}",
        env!("CARGO_PKG_VERSION"),
        date(env!("ARCH_HIERO_BUILT"))
    )
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_version_line_names_the_version_and_a_build_date() {
        let line = super::version_and_build();
        assert!(line.starts_with(&format!("Version {} · Built ", env!("CARGO_PKG_VERSION"))));
        // The build time was read as a date, not shown as the raw timestamp.
        assert!(!line.contains('T'), "{line}");
        assert!(env!("ARCH_HIERO_BUILT").parse::<jiff::Timestamp>().is_ok());
    }

    #[test]
    fn unreadable_dates_are_shown_as_they_are() {
        assert_eq!(super::date("soon"), "soon");
        assert!(super::date("2026-10-01T09:00:00.000Z").contains("2026"));
    }
}
