//! Data files extracted from keybr.com, embedded into the binary.
//!
//! See `tools/extract-keybr-data.mjs` for how they are produced.

macro_rules! language_assets {
    ($($id:literal),* $(,)?) => {
        /// Ids of the languages that have a phonetic model and a word list.
        pub const LANGUAGE_IDS: &[&str] = &[$($id),*];

        /// Returns the binary phonetic model of a language.
        pub fn model_data(id: &str) -> Option<&'static [u8]> {
            match id {
                $($id => Some(include_bytes!(concat!("../assets/models/model-", $id, ".data"))),)*
                _ => None,
            }
        }

        /// Returns the word list of a language as a JSON array of strings,
        /// most frequent words first.
        pub fn word_list_json(id: &str) -> Option<&'static str> {
            match id {
                $($id => Some(include_str!(concat!("../assets/words/words-", $id, ".json"))),)*
                _ => None,
            }
        }
    };
}

language_assets!(
    "ar", "be", "br", "cs", "da", "de", "el", "en", "en-GB", "es", "et", "fa", "fi", "fr", "he",
    "hr", "hu", "hy", "it", "ja", "lt", "lv", "nb", "nl", "pl", "pt", "ro", "ru", "sl", "sv", "th",
    "tr", "uk", "vi",
);
