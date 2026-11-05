//! Languages, ported from `@keybr/keyboard/lib/language.ts`.

use std::sync::OnceLock;

use serde::Deserialize;

use crate::assets;
use crate::unicode::cp_to_string;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Script {
    Arabic,
    Armenian,
    Cyrillic,
    Greek,
    Hebrew,
    Hiragana,
    Latin,
    Thai,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Language {
    /// ISO 639-1 language code.
    pub id: String,
    /// English name of the language.
    pub name: String,
    pub script: Script,
    pub direction: String,
    #[serde(deserialize_with = "deserialize_alphabet")]
    pub alphabet: Vec<u32>,
}

fn deserialize_alphabet<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Vec<u32>, D::Error> {
    let s = String::deserialize(d)?;
    Ok(s.chars().map(u32::from).collect())
}

impl Language {
    pub fn all() -> &'static [Language] {
        static ALL: OnceLock<Vec<Language>> = OnceLock::new();
        ALL.get_or_init(|| serde_json::from_str(assets::LANGUAGES_JSON).expect("languages.json"))
    }

    pub fn find(id: &str) -> Option<&'static Language> {
        Self::all().iter().find(|l| l.id == id)
    }

    /// Returns the language with the given id, or English.
    pub fn get(id: &str) -> &'static Language {
        Self::find(id).unwrap_or_else(|| Self::find("en").expect("english"))
    }

    pub fn is_rtl(&self) -> bool {
        self.direction == "rtl"
    }

    fn is_turkic(&self) -> bool {
        self.id == "tr"
    }

    pub fn lower_case(&self, s: &str) -> String {
        if self.is_turkic() {
            s.chars()
                .map(|c| match c {
                    'I' => "ı".to_string(),
                    'İ' => "i".to_string(),
                    c => c.to_lowercase().collect(),
                })
                .collect()
        } else {
            s.to_lowercase()
        }
    }

    pub fn upper_case(&self, s: &str) -> String {
        if self.is_turkic() {
            s.chars()
                .map(|c| match c {
                    'i' => "İ".to_string(),
                    'ı' => "I".to_string(),
                    c => c.to_uppercase().collect(),
                })
                .collect()
        } else {
            s.to_uppercase()
        }
    }

    /// Uppercases the first letter and lowercases the rest.
    pub fn capital_case(&self, s: &str) -> String {
        let mut chars = s.chars();
        match chars.next() {
            Some(first) => {
                let mut out = self.upper_case(&first.to_string());
                out.push_str(&self.lower_case(chars.as_str()));
                out
            }
            None => String::new(),
        }
    }

    /// Checks whether the string consists only of the letters of this language.
    pub fn test(&self, s: &str) -> bool {
        self.lower_case(s)
            .chars()
            .all(|c| self.alphabet.contains(&u32::from(c)))
    }

    /// Returns a value indicating whether the code point can be a letter of this language.
    pub fn includes(&self, cp: u32) -> bool {
        match self.script {
            Script::Arabic => (0x0600..=0x06ff).contains(&cp),
            Script::Armenian => (0x0530..=0x058f).contains(&cp),
            Script::Cyrillic => (0x0400..=0x04ff).contains(&cp),
            Script::Greek => (0x0370..=0x03ff).contains(&cp),
            Script::Hebrew => (0x0590..=0x05ff).contains(&cp),
            Script::Hiragana => (0x3040..=0x309f).contains(&cp),
            Script::Latin => {
                (0x0041..=0x005a).contains(&cp)
                    || (0x0061..=0x007a).contains(&cp)
                    || (0x00c0..=0x00d6).contains(&cp)
                    || (0x00d8..=0x00f6).contains(&cp)
                    || (0x00f8..=0x00ff).contains(&cp)
                    || (0x0100..=0x017f).contains(&cp)
                    || (0x0180..=0x024f).contains(&cp)
            }
            Script::Thai => (0x0e00..=0x0e7f).contains(&cp),
        }
    }

    /// Returns the display label of a letter, such as the uppercase variant.
    pub fn letter_name(&self, cp: u32) -> String {
        match cp {
            0x00df => "ẞ".into(),
            0x0587 => "ԵՎ".into(),
            0x0590..=0x05ff => cp_to_string(cp),
            // The original prepends a zero width non-joiner to break Arabic
            // ligatures, terminals render isolated letters already.
            0x0600..=0x06ff => cp_to_string(cp),
            0x0e31 | 0x0e34..=0x0e3a | 0x0e47..=0x0e4e => format!("◌{}", cp_to_string(cp)),
            0x0e00..=0x0e7f => cp_to_string(cp),
            _ => self.upper_case(&cp_to_string(cp)),
        }
    }
}
