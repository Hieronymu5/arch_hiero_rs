//! Text measurement and wrapping. Measurement shapes text with the bundled Inter font using
//! `harfrust`, the shaper egui uses, so widths agree between the editor, the exports, and the
//! MCP server.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use harfrust::{FontRef, ShapeOptions, ShaperData, UnicodeBuffer};
use skrifa::MetadataProvider;
use skrifa::instance::{LocationRef, Size as FontSize};

/// The bundled fonts, also registered with egui so the editor draws what was measured.
pub const FONT_REGULAR: &[u8] = include_bytes!("../assets/fonts/Inter-Regular.ttf");
pub const FONT_SEMIBOLD: &[u8] = include_bytes!("../assets/fonts/Inter-SemiBold.ttf");
pub const FONT_BOLD: &[u8] = include_bytes!("../assets/fonts/Inter-Bold.ttf");

/// The font family written into exported SVG files: the bundled font first, then the system's.
pub const FONT_FAMILY: &str =
    "Inter, system-ui, -apple-system, 'Segoe UI', Roboto, Helvetica, Arial, sans-serif";

/// A font weight the app uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Weight {
    Regular,
    SemiBold,
    Bold,
}

impl Weight {
    /// The CSS weight number.
    pub const fn css(self) -> u16 {
        match self {
            Self::Regular => 400,
            Self::SemiBold => 600,
            Self::Bold => 700,
        }
    }
}

/// Measures how wide text is, in pixels.
pub trait MeasureText: Send + Sync {
    fn width(&self, text: &str, size: f64, weight: Weight) -> f64;
}

/// A rough measurement: an average glyph width. Used by tests so results don't depend on font
/// details, and matches the prototype's fallback.
#[derive(Debug, Default, Clone, Copy)]
pub struct EstimateText;

impl MeasureText for EstimateText {
    fn width(&self, text: &str, size: f64, _weight: Weight) -> f64 {
        text.chars().count() as f64 * size * 0.55
    }
}

struct LoadedFont {
    font: FontRef<'static>,
    data: ShaperData,
    units_per_em: f64,
    ascent: f64,
    descent: f64,
}

impl LoadedFont {
    fn load(bytes: &'static [u8]) -> Self {
        let font = FontRef::new(bytes).expect("bundled font parses");
        let metrics = font.metrics(FontSize::unscaled(), LocationRef::default());
        let data = ShaperData::new(&font);
        Self {
            units_per_em: f64::from(metrics.units_per_em),
            ascent: f64::from(metrics.ascent),
            descent: f64::from(metrics.descent),
            font,
            data,
        }
    }

    /// The width of `text` at one pixel per em.
    fn unit_width(&self, text: &str) -> f64 {
        let shaper = self.data.shaper(&self.font).build();
        let mut buffer = UnicodeBuffer::new();
        buffer.push_str(text);
        buffer.guess_segment_properties();
        let glyphs = shaper.shape(buffer, ShapeOptions::new());
        let advance: i64 = glyphs
            .glyph_positions()
            .iter()
            .map(|p| i64::from(p.x_advance))
            .sum();
        advance as f64 / self.units_per_em
    }
}

/// Measures text in the bundled Inter font, caching widths.
pub struct FontMetrics {
    regular: LoadedFont,
    semibold: LoadedFont,
    bold: LoadedFont,
    cache: Mutex<HashMap<(Weight, String), f64>>,
}

impl FontMetrics {
    fn font(&self, weight: Weight) -> &LoadedFont {
        match weight {
            Weight::Regular => &self.regular,
            Weight::SemiBold => &self.semibold,
            Weight::Bold => &self.bold,
        }
    }

    /// How far glyphs rise above the baseline, as a fraction of the font size.
    pub fn ascent(&self, weight: Weight) -> f64 {
        let f = self.font(weight);
        f.ascent / f.units_per_em
    }

    /// How far glyphs drop below the baseline, as a (negative) fraction of the font size.
    pub fn descent(&self, weight: Weight) -> f64 {
        let f = self.font(weight);
        f.descent / f.units_per_em
    }
}

impl MeasureText for FontMetrics {
    fn width(&self, text: &str, size: f64, weight: Weight) -> f64 {
        if text.is_empty() {
            return 0.0;
        }
        let key = (weight, text.to_owned());
        let mut cache = self
            .cache
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(width) = cache.get(&key) {
            return width * size;
        }
        let width = self.font(weight).unit_width(text);
        if cache.len() > 50_000 {
            cache.clear();
        }
        cache.insert(key, width);
        width * size
    }
}

/// The shared font measurer.
pub fn font_metrics() -> &'static FontMetrics {
    static METRICS: OnceLock<FontMetrics> = OnceLock::new();
    METRICS.get_or_init(|| FontMetrics {
        regular: LoadedFont::load(FONT_REGULAR),
        semibold: LoadedFont::load(FONT_SEMIBOLD),
        bold: LoadedFont::load(FONT_BOLD),
        cache: Mutex::new(HashMap::new()),
    })
}

/// A text style: font size, weight, line height, and the most lines shown.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TextStyle {
    pub size: f64,
    pub weight: Weight,
    pub line_height: f64,
    pub max_lines: usize,
}

impl TextStyle {
    pub const fn new(size: f64, weight: Weight, line_height: f64, max_lines: usize) -> Self {
        Self {
            size,
            weight,
            line_height,
            max_lines,
        }
    }

    pub const fn with_max_lines(self, max_lines: usize) -> Self {
        Self { max_lines, ..self }
    }

    pub const fn with_line_height(self, line_height: f64) -> Self {
        Self {
            line_height,
            ..self
        }
    }
}

/// Splits text into lines no wider than `width`, breaking between words (and inside a word
/// that's too long on its own), and at each line break in it. Text beyond `max_lines` is cut,
/// ending the last line with "…".
pub fn wrap_text(
    text: &str,
    width: f64,
    style: TextStyle,
    measure: &dyn MeasureText,
) -> Vec<String> {
    let fits = |line: &str| measure.width(line, style.size, style.weight) <= width;
    let lines: Vec<String> = text
        .trim()
        .lines()
        .flat_map(|paragraph| wrap_paragraph(paragraph, &fits))
        .collect();
    if lines.len() <= style.max_lines {
        return lines;
    }
    let mut kept: Vec<String> = lines.into_iter().take(style.max_lines).collect();
    if let Some(last) = kept.last_mut() {
        let mut chars: Vec<char> = last.chars().collect();
        while !chars.is_empty() && !fits(&format!("{}…", chars.iter().collect::<String>())) {
            chars.pop();
            while chars.last().is_some_and(|c| c.is_whitespace()) {
                chars.pop();
            }
        }
        *last = format!("{}…", chars.iter().collect::<String>());
    }
    kept
}

/// Splits text with no line breaks into lines that fit. An empty one is an empty line.
fn wrap_paragraph(text: &str, fits: &dyn Fn(&str) -> bool) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    let mut line = String::new();
    for word in text.split_whitespace() {
        let candidate = if line.is_empty() {
            word.to_owned()
        } else {
            format!("{line} {word}")
        };
        if fits(&candidate) {
            line = candidate;
            continue;
        }
        if !line.is_empty() {
            lines.push(std::mem::take(&mut line));
        }
        let mut rest: Vec<char> = word.chars().collect();
        while rest.len() > 1 && !fits(&rest.iter().collect::<String>()) {
            let mut cut = rest.len() - 1;
            while cut > 1 && !fits(&rest[..cut].iter().collect::<String>()) {
                cut -= 1;
            }
            lines.push(rest[..cut].iter().collect());
            rest = rest[cut..].to_vec();
        }
        line = rest.into_iter().collect();
    }
    if !line.is_empty() || lines.is_empty() {
        lines.push(line);
    }
    lines
}

/// Truncates `text` with "…" so it fits in `width`, as the deployed component exports do.
pub fn fit_text(
    text: &str,
    width: f64,
    size: f64,
    weight: Weight,
    measure: &dyn MeasureText,
) -> String {
    if measure.width(text, size, weight) <= width {
        return text.to_owned();
    }
    let mut chars: Vec<char> = text.chars().collect();
    while !chars.is_empty() {
        chars.pop();
        let candidate = format!("{}…", chars.iter().collect::<String>().trim_end());
        if measure.width(&candidate, size, weight) <= width {
            return candidate;
        }
    }
    String::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    const STYLE: TextStyle = TextStyle::new(10.0, Weight::Regular, 12.0, 2);

    #[test]
    fn wraps_between_words() {
        // Each character is 5.5 px wide with the estimate.
        let lines = wrap_text("alpha beta gamma", 60.0, STYLE, &EstimateText);
        assert_eq!(lines, vec!["alpha beta", "gamma"]);
    }

    #[test]
    fn cuts_with_an_ellipsis() {
        let lines = wrap_text("one two three four five six", 40.0, STYLE, &EstimateText);
        assert_eq!(lines.len(), 2);
        assert!(lines[1].ends_with('…'));
    }

    #[test]
    fn starts_a_new_line_at_each_line_break_keeping_empty_lines() {
        let lines = wrap_text(
            "one\ntwo three\r\n\nfour",
            90.0,
            STYLE.with_max_lines(4),
            &EstimateText,
        );
        assert_eq!(lines, vec!["one", "two three", "", "four"]);
        assert!(wrap_text("\n", 90.0, STYLE, &EstimateText).is_empty());
        assert!(wrap_text("", 90.0, STYLE, &EstimateText).is_empty());
    }

    #[test]
    fn breaks_long_words() {
        let lines = wrap_text(
            "abcdefghijklmnop",
            30.0,
            STYLE.with_max_lines(10),
            &EstimateText,
        );
        assert_eq!(lines, vec!["abcde", "fghij", "klmno", "p"]);
    }

    #[test]
    fn font_metrics_measure_real_widths() {
        let m = font_metrics();
        let narrow = m.width("iiii", 16.0, Weight::Regular);
        let wide = m.width("WWWW", 16.0, Weight::Regular);
        assert!(narrow > 0.0 && wide > narrow * 2.0);
        assert!(m.width("Server", 16.0, Weight::Bold) > m.width("Server", 16.0, Weight::Regular));
        assert!(m.ascent(Weight::Regular) > 0.5 && m.ascent(Weight::Regular) < 1.2);
    }

    #[test]
    fn fit_text_truncates() {
        assert_eq!(
            fit_text("abcdef", 100.0, 10.0, Weight::Regular, &EstimateText),
            "abcdef"
        );
        let cut = fit_text("abcdefghij", 30.0, 10.0, Weight::Regular, &EstimateText);
        assert!(cut.ends_with('…') && cut.chars().count() <= 5);
    }
}
