//! Color fields (S5.8.3 edge case): `#RGB`, `#RRGGBB` or a color name, case-insensitive, and
//! nothing else. A field keeps what it accepts as `#RRGGBB`.

/// The color names a field takes, as `0xRRGGBB` (CSS's basic colors, and the usual spellings).
const NAMES: [(&str, u32); 20] = [
    ("white", 0xFF_FFFF),
    ("black", 0x00_0000),
    ("red", 0xFF_0000),
    ("green", 0x00_8000),
    ("lime", 0x00_FF00),
    ("blue", 0x00_00FF),
    ("navy", 0x00_0080),
    ("yellow", 0xFF_FF00),
    ("orange", 0xFF_A500),
    ("cyan", 0x00_FFFF),
    ("aqua", 0x00_FFFF),
    ("teal", 0x00_8080),
    ("magenta", 0xFF_00FF),
    ("fuchsia", 0xFF_00FF),
    ("purple", 0x80_0080),
    ("pink", 0xFF_C0CB),
    ("brown", 0xA5_2A2A),
    ("silver", 0xC0_C0C0),
    ("gray", 0x80_8080),
    ("grey", 0x80_8080),
];

/// The color `text` names, as `0xRRGGBB`: `#RGB`, `#RRGGBB` or a color name.
pub fn hex_color(text: &str) -> Option<u32> {
    let text = text.trim();
    if let Some(digits) = text.strip_prefix('#') {
        return hex_digits(digits);
    }
    NAMES.iter().find(|(name, _)| name.eq_ignore_ascii_case(text)).map(|(_, color)| *color)
}

/// `color` as a field keeps it: `#RRGGBB`.
pub fn format(color: u32) -> String {
    format!("#{color:06X}")
}

/// Three or six hex digits as a color; `F80` is `FF8800`.
fn hex_digits(digits: &str) -> Option<u32> {
    if !digits.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    match digits.len() {
        6 => u32::from_str_radix(digits, 16).ok(),
        3 => {
            let doubled: String = digits.chars().flat_map(|c| [c, c]).collect();
            u32::from_str_radix(&doubled, 16).ok()
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_in_three_or_six_digits_and_names_are_colors() {
        assert_eq!(hex_color("#FFa500"), Some(0xFF_A500));
        assert_eq!(hex_color("#f80"), Some(0xFF_8800));
        assert_eq!(hex_color(" Yellow "), Some(0xFF_FF00));
        assert_eq!(format(0xFF_8800), "#FF8800");
    }

    #[test]
    fn anything_else_is_refused() {
        for text in ["FFA500", "#FFFF", "#GGGGGG", "", "#", "chartreuse-ish", "rgb(1,2,3)"] {
            assert_eq!(hex_color(text), None, "{text}");
        }
    }
}
