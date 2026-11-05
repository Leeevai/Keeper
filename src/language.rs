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
}
