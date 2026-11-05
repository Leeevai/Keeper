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
