//! Headless preview support shared by the showcase and its tests.
//! Captures the renderer's output commands, including retained frame state.
use jilva::{Buffer, OutputCommand, ResolvedAttributes};
use std::{fmt::Write as _, path::Path};

pub struct Snapshot {
    pub buffer: Buffer,
    cursor: (u16, u16),
    attrs: ResolvedAttributes,
}

impl Snapshot {
    pub fn new((width, height): (u16, u16)) -> Self {
        Self {
            buffer: Buffer::new(width, height),
            cursor: (0, 0),
            attrs: ResolvedAttributes::plain(),
        }
    }

    pub fn apply(&mut self, commands: &[OutputCommand]) {
        for command in commands {
            match command {
                OutputCommand::MoveTo { x, y } => self.cursor = (*x, *y),
                OutputCommand::SetAttributes(attrs) => self.attrs = *attrs,
                OutputCommand::ResetAttributes => self.attrs = ResolvedAttributes::plain(),
                OutputCommand::PrintSegment { text } => {
                    self.cursor.0 =
                        self.buffer
                            .set_string(self.cursor.0, self.cursor.1, text, self.attrs)
                }
                OutputCommand::ClearRegion { rect } => {
                    self.buffer.fill(*rect, ResolvedAttributes::plain())
                }
                _ => {}
            }
        }
    }

    pub fn save(&self, dir: &Path, name: &str, light: bool) -> std::io::Result<()> {
        std::fs::create_dir_all(dir)?;
        std::fs::write(
            dir.join(format!("{name}.txt")),
            self.buffer.text_rows().join("\n"),
        )?;
        std::fs::write(dir.join(format!("{name}.svg")), self.svg(light))
    }

    fn svg(&self, light: bool) -> String {
        // Explicit assumptions for terminal-default colours and ANSI colours:
        // an emulator's user palette and font can produce different pixels.
        let default_fg = if light { (20, 20, 20) } else { (245, 245, 245) };
        let default_bg = if light { (248, 248, 248) } else { (0, 0, 0) };
        let mut svg = format!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{}\" height=\"{}\" viewBox=\"0 0 {} {}\"><title>Jilva Studio terminal frame</title><g font-family=\"Cascadia Mono, DejaVu Sans Mono, monospace\" font-size=\"20\" xml:space=\"preserve\">",
            self.buffer.width() as u32 * 12,
            self.buffer.height() as u32 * 24,
            self.buffer.width() as u32 * 12,
            self.buffer.height() as u32 * 24,
        );
        for y in 0..self.buffer.height() {
            let row = self.buffer.row(y).expect("row in bounds");
            let mut x = 0usize;
            while x < row.len() {
                let attrs = row[x].attrs();
                let mut end = x + 1;
                while end < row.len() && row[end].attrs() == attrs {
                    end += 1;
                }
                let (mut fg, mut bg) = (attrs.fg.to_rgb(default_fg), attrs.bg.to_rgb(default_bg));
                if attrs.reverse {
                    std::mem::swap(&mut fg, &mut bg);
                }
                let _ = write!(
                    svg,
                    "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"24\" fill=\"{}\"/>",
                    x * 12,
                    u32::from(y) * 24,
                    (end - x) * 12,
                    hex(bg)
                );
                let content = row[x..end]
                    .iter()
                    .map(|cell| cell.symbol())
                    .collect::<String>();
                if !content.trim().is_empty() {
                    let decoration = if attrs.underline {
                        "underline"
                    } else if attrs.strikethrough {
                        "line-through"
                    } else {
                        "none"
                    };
                    let _ = write!(
                        svg,
                        "<text y=\"{}\" fill=\"{}\" font-weight=\"{}\" font-style=\"{}\" text-decoration=\"{}\" opacity=\"{}\">",
                        u32::from(y) * 24 + 19,
                        hex(fg),
                        if attrs.bold { 600 } else { 400 },
                        if attrs.italic { "italic" } else { "normal" },
                        decoration,
                        if attrs.dim { "0.65" } else { "1" },
                    );
                    // Anchor each grapheme to its terminal column. SVG text
                    // whitespace and fallback font advances must not move
                    // borders, wide glyphs or right-aligned content.
                    for (offset, cell) in row[x..end].iter().enumerate() {
                        if cell.symbol().trim().is_empty() {
                            continue;
                        }
                        let _ = write!(
                            svg,
                            "<tspan x=\"{}\" textLength=\"{}\" lengthAdjust=\"spacingAndGlyphs\">{}</tspan>",
                            (x + offset) * 12,
                            u32::from(cell.width()) * 12,
                            escape(cell.symbol()),
                        );
                    }
                    svg.push_str("</text>");
                }
                x = end;
            }
        }
        svg.push_str("</g></svg>\n");
        svg
    }
}

fn hex((r, g, b): (u8, u8, u8)) -> String {
    format!("#{r:02x}{g:02x}{b:02x}")
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

#[cfg(test)]
mod tests {
    use super::*;
    use jilva::ResolvedColor;

    #[test]
    fn svg_escapes_text_and_applies_reverse_video() {
        let mut frame = Snapshot::new((4, 1));
        let mut attrs =
            ResolvedAttributes::new(ResolvedColor::Rgb(1, 2, 3), ResolvedColor::Rgb(4, 5, 6));
        attrs.reverse = true;
        frame.buffer.set_string(0, 0, "<&>", attrs);
        let svg = frame.svg(false);
        for entity in ["&lt;", "&amp;", "&gt;"] {
            assert!(svg.contains(entity));
        }
        assert!(svg.contains("fill=\"#010203\""));
        assert!(svg.contains("fill=\"#040506\""));
        assert!(!svg.contains("<&>"));
    }

    #[test]
    fn svg_anchors_each_grapheme_to_its_terminal_columns() {
        let mut frame = Snapshot::new((8, 1));
        frame
            .buffer
            .set_string(1, 0, "a 日é", ResolvedAttributes::plain());
        let svg = frame.svg(false);
        for expected in [
            "x=\"12\" textLength=\"12\" lengthAdjust=\"spacingAndGlyphs\">a</tspan>",
            "x=\"36\" textLength=\"24\" lengthAdjust=\"spacingAndGlyphs\">日</tspan>",
            "x=\"60\" textLength=\"12\" lengthAdjust=\"spacingAndGlyphs\">é</tspan>",
        ] {
            assert!(svg.contains(expected), "missing {expected}");
        }
        assert_eq!(svg.matches("<tspan ").count(), 3);
    }
}
