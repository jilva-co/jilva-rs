//! Grapheme segmentation and cell-width authorities (RFC-0004 §4).
//!
//! Two external tables decide everything here, and nothing else in the
//! workspace may second-guess them:
//!
//! - **Segmentation:** `unicode-segmentation` extended grapheme clusters
//!   (UAX #29). A user-perceived character is one cluster, never one `char`.
//! - **Width:** `unicode-width` without its `cjk` feature (UAX #11,
//!   East-Asian-Ambiguous = 1 cell), clamped to the range `0..=2` a terminal
//!   cell grid can express.
//!
//! Terminals may disagree with these tables. The renderer states the table it
//! trusts and offers [`crate::renderer::DiffOptions::resync_after_wide`] as the
//! mitigation; it never probes the terminal mid-frame.
//!
//! A cluster that contains any control character (C0, DEL, C1) has width `0`
//! and is *skipped* by every text-writing function. Controls never reach a
//! cell, so they can never reach [`crate::OutputCommand::PrintSegment`].

use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

/// Returns the number of terminal cells one grapheme cluster occupies.
///
/// Result is `0` (nothing to draw: empty, control-bearing, or a zero-width
/// cluster such as a lone combining mark), `1`, or `2`. The input is treated
/// as a single cluster; use [`graphemes`] to segment text first.
///
/// # Example
///
/// ```rust
/// use jilva_core::renderer::cluster_width;
///
/// assert_eq!(cluster_width("a"), 1);
/// assert_eq!(cluster_width("e\u{301}"), 1); // e + combining acute
/// assert_eq!(cluster_width("日"), 2);
/// assert_eq!(cluster_width("👩‍💻"), 2); // ZWJ sequence is one cluster
/// assert_eq!(cluster_width("\u{1b}"), 0); // controls are never drawn
/// ```
#[must_use]
pub fn cluster_width(cluster: &str) -> u8 {
    if cluster.chars().any(char::is_control) {
        return 0;
    }
    match cluster.width() {
        0 => 0,
        1 => 1,
        _ => 2,
    }
}

/// Segments text into extended grapheme clusters.
///
/// Thin, documented seam over the segmentation authority so the rest of the
/// crate never names the external crate.
///
/// # Example
///
/// ```rust
/// use jilva_core::renderer::graphemes;
///
/// let clusters: Vec<&str> = graphemes("ae\u{301}🇺🇿").collect();
/// assert_eq!(clusters, ["a", "e\u{301}", "🇺🇿"]);
/// ```
pub fn graphemes(text: &str) -> impl Iterator<Item = &str> {
    text.graphemes(true)
}

/// Returns the number of terminal cells `text` occupies on one row.
///
/// Sums [`cluster_width`] over the grapheme clusters, so control-bearing and
/// zero-width clusters add nothing — exactly the cells the buffer would draw.
///
/// # Example
///
/// ```rust
/// use jilva_core::renderer::text_width;
///
/// assert_eq!(text_width("hello"), 5);
/// assert_eq!(text_width("日本語"), 6);
/// assert_eq!(text_width("a\tb"), 2); // the tab is skipped, not expanded
/// ```
#[must_use]
pub fn text_width(text: &str) -> usize {
    graphemes(text)
        .map(|cluster| usize::from(cluster_width(cluster)))
        .sum()
}

/// Returns the longest prefix of `text` that fits in `max_cols` cells and the
/// number of cells that prefix occupies.
///
/// Never splits a grapheme cluster and never splits a double-width cell: a
/// wide cluster that would straddle the limit is excluded, so the returned
/// width can be `max_cols - 1` (the one-column-remainder edge). Skipped
/// (zero-width) clusters inside the prefix are kept in the returned slice.
///
/// # Example
///
/// ```rust
/// use jilva_core::renderer::truncate_to_width;
///
/// assert_eq!(truncate_to_width("hello", 3), ("hel", 3));
/// // Two wide cells need 4 columns; 3 columns fit only one of them.
/// assert_eq!(truncate_to_width("日本", 3), ("日", 2));
/// assert_eq!(truncate_to_width("日本", 1), ("", 0));
/// ```
#[must_use]
pub fn truncate_to_width(text: &str, max_cols: usize) -> (&str, usize) {
    let mut used = 0usize;
    let mut end = 0usize;
    for (offset, cluster) in text.grapheme_indices(true) {
        let width = usize::from(cluster_width(cluster));
        if used + width > max_cols {
            break;
        }
        used += width;
        end = offset + cluster.len();
    }
    (&text[..end], used)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn widths_follow_the_documented_authority() {
        for (text, expected) in [
            ("a", 1),
            ("~", 1),
            ("é", 1),
            ("e\u{301}", 1),
            ("日", 2),
            ("한", 2),
            ("👍", 2),
            ("👩‍💻", 2),
            ("🇺🇿", 2),
            ("❤\u{fe0f}", 2),
            ("\u{200b}", 0),
            ("\u{301}", 0),
            ("", 0),
        ] {
            assert_eq!(cluster_width(text), expected, "width of {text:?}");
        }
    }

    #[test]
    fn controls_are_zero_width_even_inside_a_cluster() {
        for codepoint in (0..=0x1f).chain(0x7f..=0x9f) {
            let control = char::from_u32(codepoint).unwrap();
            assert_eq!(cluster_width(&control.to_string()), 0);
            assert_eq!(cluster_width(&format!("a{control}")), 0);
        }
        // Only the ESC is skipped; the rest of the "sequence" is plain text.
        assert_eq!(text_width("\u{1b}[31mred"), 7);
    }

    #[test]
    fn text_width_counts_cells_not_chars_or_bytes() {
        assert_eq!(text_width("hello"), 5);
        assert_eq!(text_width("日本語"), 6);
        assert_eq!(text_width("a👩‍💻b"), 4);
        assert_eq!("a👩‍💻b".chars().count(), 5);
        assert_eq!(text_width(""), 0);
    }

    #[test]
    fn truncation_never_splits_a_cluster_or_wide_cell() {
        assert_eq!(truncate_to_width("hello", 0), ("", 0));
        assert_eq!(truncate_to_width("hello", 99), ("hello", 5));
        assert_eq!(truncate_to_width("日本", 4), ("日本", 4));
        assert_eq!(truncate_to_width("日本", 3), ("日", 2));
        assert_eq!(truncate_to_width("a日", 2), ("a", 1));
        assert_eq!(truncate_to_width("e\u{301}x", 1), ("e\u{301}", 1));
        assert_eq!(truncate_to_width("👩‍💻x", 1), ("", 0));
        // Property: the prefix always fits and always ends on a boundary.
        let sample = "a日e\u{301}👩‍💻🇺🇿z\u{301}한";
        for limit in 0..=text_width(sample) + 2 {
            let (prefix, used) = truncate_to_width(sample, limit);
            assert!(used <= limit);
            assert_eq!(text_width(prefix), used);
            assert!(sample.starts_with(prefix));
            assert!(sample.is_char_boundary(prefix.len()));
        }
    }
}
