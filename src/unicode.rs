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
