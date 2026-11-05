//! Text normalization, ported from `@keybr/unicode` and `@keybr/keyboard`.

use std::collections::HashMap;
use std::sync::OnceLock;

use unicode_normalization::UnicodeNormalization;
use unicode_normalization::char::compose;

use crate::assets;

pub const SPACE: u32 = 0x0020;

pub fn is_control(cp: u32) -> bool {
    cp < 0x0020
}

pub fn is_linebreak(cp: u32) -> bool {
    matches!(cp, 0x000a | 0x000b | 0x000c | 0x000d | 0x2028 | 0x2029)
}

pub fn is_whitespace(cp: u32) -> bool {
    matches!(
        cp,
        0x0009 | 0x000a | 0x000b | 0x000c | 0x000d | 0x0020 | 0x0085 | 0x00a0 | 0x1680 | 0x2000
            ..=0x200a | 0x2028 | 0x2029 | 0x202f | 0x205f | 0x3000
    )
}

pub fn is_diacritic(cp: u32) -> bool {
    (0x0300..=0x036f).contains(&cp)
}

/// Combines a base letter with a combining diacritic into a single code point,
/// or returns the base letter if there is no such composition.
pub fn combine_diacritic(base: u32, combining: u32) -> u32 {
    match (char::from_u32(base), char::from_u32(combining)) {
        (Some(a), Some(b)) => compose(a, b).map_or(base, u32::from),
        _ => base,
    }
}

pub fn to_code_points(s: &str) -> Vec<u32> {
    s.chars().map(u32::from).collect()
}

pub fn from_code_points(cps: &[u32]) -> String {
    cps.iter().filter_map(|&cp| char::from_u32(cp)).collect()
}

pub fn cp_to_string(cp: u32) -> String {
    char::from_u32(cp).map(String::from).unwrap_or_default()
}

struct NormalizeTables {
    expand: HashMap<u32, Vec<u32>>,
    replace: HashMap<u32, u32>,
}

fn tables() -> &'static NormalizeTables {
    static TABLES: OnceLock<NormalizeTables> = OnceLock::new();
    TABLES.get_or_init(|| {
        #[derive(serde::Deserialize)]
        struct Raw {
            expand: Vec<(Vec<u32>, Vec<u32>)>,
            replace: Vec<(u32, Vec<u32>)>,
        }
        let raw: Raw = serde_json::from_str(assets::NORMALIZE_JSON).expect("normalize.json");
        let mut expand = HashMap::new();
        for (to, from) in raw.expand {
            for cp in from {
                expand.insert(cp, to.clone());
            }
        }
        let mut replace = HashMap::new();
        for (to, from) in raw.replace {
            for cp in from {
                replace.insert(cp, to);
            }
        }
        NormalizeTables { expand, replace }
    })
}

/// Maps typographic variants of a character, such as curly quotes,
/// to the character found on a keyboard.
pub fn normalize_cp(cp: u32) -> u32 {
    tables().replace.get(&cp).copied().unwrap_or(cp)
}

/// Removes characters that are not in the given set from the text,
/// replacing accented letters with their plain variants when possible,
/// and collapses whitespace.
pub fn filter_text(text: &str, allowed: impl Fn(u32) -> bool) -> String {
    let tables = tables();
    let mut result = String::new();
    let mut ws: Option<char> = None;

    fn append(result: &mut String, ws: &mut Option<char>, ch: char) {
        if let Some(w) = ws.take() {
            result.push(w);
        }
        result.push(ch);
    }
    fn space(result: &str, ws: &mut Option<char>, ch: char) {
        if !result.is_empty() && *ws != Some('\n') {
            *ws = Some(ch);
        }
    }

    for ch in text.nfc() {
        let cp = u32::from(ch);
        if is_linebreak(cp) {
            space(&result, &mut ws, '\n');
            continue;
        }
        if is_whitespace(cp) || is_control(cp) {
            space(&result, &mut ws, ' ');
            continue;
        }
        if is_diacritic(cp) {
            continue;
        }
        if allowed(cp) {
            append(&mut result, &mut ws, ch);
            continue;
        }
        if let Some(letters) = tables.expand.get(&cp) {
            for &letter in letters {
                if allowed(letter)
                    && let Some(c) = char::from_u32(letter)
                {
                    append(&mut result, &mut ws, c);
                }
            }
            continue;
        }
        if let Some(&replacement) = tables.replace.get(&cp) {
            if allowed(replacement) {
                // Keep the original typographic character, it will be
                // accepted as the replacement while typing.
                append(&mut result, &mut ws, ch);
            } else {
                space(&result, &mut ws, ' ');
            }
            continue;
        }
        space(&result, &mut ws, ' ');
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ascii(cp: u32) -> bool {
        (0x21..0x7f).contains(&cp)
    }

    #[test]
    fn filters_and_collapses_whitespace() {
        assert_eq!(filter_text("  hello \t  world  ", ascii), "hello world");
        assert_eq!(filter_text("a\n\n b", ascii), "a\nb");
        assert_eq!(filter_text("a☃b", ascii), "a b");
        // A dash can be typed with the hyphen key, so it is kept.
        assert_eq!(filter_text("a—b", ascii), "a—b");
        assert_eq!(filter_text("abc", |cp| cp != u32::from('b')), "a c");
    }

    #[test]
    fn expands_accented_letters() {
        assert_eq!(filter_text("café", ascii), "cafe");
        assert_eq!(filter_text("Æsir", ascii), "AEsir");
        // Decomposed input is composed first.
        assert_eq!(filter_text("cafe\u{0301}", ascii), "cafe");
    }

    #[test]
    fn keeps_typographic_quotes_that_can_be_typed() {
        assert_eq!(filter_text("“hi”", ascii), "“hi”");
        assert_eq!(normalize_cp(u32::from('“')), u32::from('"'));
        assert_eq!(normalize_cp(u32::from('’')), u32::from('\''));
        assert_eq!(normalize_cp(u32::from('a')), u32::from('a'));
        assert_eq!(
            filter_text("“hi”", |cp| cp != u32::from('"') && ascii(cp)),
            "hi"
        );
    }

    #[test]
    fn combines_diacritics() {
        assert_eq!(combine_diacritic(u32::from('e'), 0x0301), u32::from('é'));
        assert_eq!(combine_diacritic(u32::from('q'), 0x0301), u32::from('q'));
    }
}
