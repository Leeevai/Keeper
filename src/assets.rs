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

/// Returns the scrambled list of words that the generator must never produce.
pub fn blacklist_json(id: &str) -> Option<&'static str> {
    match id {
        "de" => Some(include_str!("../assets/blacklist/blacklist-de.json")),
        "en" => Some(include_str!("../assets/blacklist/blacklist-en.json")),
        "es" => Some(include_str!("../assets/blacklist/blacklist-es.json")),
        "fr" => Some(include_str!("../assets/blacklist/blacklist-fr.json")),
        "it" => Some(include_str!("../assets/blacklist/blacklist-it.json")),
        "nl" => Some(include_str!("../assets/blacklist/blacklist-nl.json")),
        "pt" => Some(include_str!("../assets/blacklist/blacklist-pt.json")),
        "ru" => Some(include_str!("../assets/blacklist/blacklist-ru.json")),
        "sv" => Some(include_str!("../assets/blacklist/blacklist-sv.json")),
        "uk" => Some(include_str!("../assets/blacklist/blacklist-uk.json")),
        _ => None,
    }
}

pub const LANGUAGES_JSON: &str = include_str!("../assets/keyboard/languages.json");
pub const LAYOUTS_JSON: &str = include_str!("../assets/keyboard/layouts.json");
pub const GEOMETRIES_JSON: &str = include_str!("../assets/keyboard/geometries.json");
pub const NORMALIZE_JSON: &str = include_str!("../assets/unicode/normalize.json");
pub const BOOKS_JSON: &str = include_str!("../assets/books/books.json");
pub const SYNTAXES_JSON: &str = include_str!("../assets/code/syntaxes.json");
pub const DIST_SPEED_JSON: &str = include_str!("../assets/dist/dist_speed.json");
pub const DIST_ACCURACY_JSON: &str = include_str!("../assets/dist/dist_accuracy.json");

/// Returns the content of a book: a JSON array of `[chapter title, [paragraphs]]`.
pub fn book_json(id: &str) -> Option<&'static str> {
    match id {
        "en-alice-wonderland" => Some(include_str!("../assets/books/en-alice-wonderland.json")),
        "en-jekyll-hyde" => Some(include_str!("../assets/books/en-jekyll-hyde.json")),
        "en-call-wild" => Some(include_str!("../assets/books/en-call-wild.json")),
        "es-marianela" => Some(include_str!("../assets/books/es-marianela.json")),
        "de-alice-wonderland" => Some(include_str!("../assets/books/de-alice-wonderland.json")),
        "fr-alice-wonderland" => Some(include_str!("../assets/books/fr-alice-wonderland.json")),
        _ => None,
    }
}
