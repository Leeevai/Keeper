//! Word lists and books, ported from `@keybr/content`.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use serde::Deserialize;

use crate::assets;

#[derive(Debug, Clone, Deserialize)]
pub struct Book {
    pub id: String,
    pub language: String,
    pub title: String,
    pub author: String,
}

impl Book {
    pub fn all() -> &'static [Book] {
        static ALL: OnceLock<Vec<Book>> = OnceLock::new();
        ALL.get_or_init(|| serde_json::from_str(assets::BOOKS_JSON).expect("books.json"))
    }

    pub fn find(id: &str) -> Option<&'static Book> {
        Self::all().iter().find(|b| b.id == id)
    }

    pub fn get(id: &str) -> &'static Book {
        Self::find(id).unwrap_or(&Self::all()[0])
    }

    /// The paragraphs of all chapters.
    pub fn paragraphs(&self) -> &'static [String] {
        static CACHE: OnceLock<Mutex<HashMap<String, &'static [String]>>> = OnceLock::new();
        let mut cache = CACHE
            .get_or_init(Default::default)
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        cache.entry(self.id.clone()).or_insert_with(|| {
            let chapters: Vec<(String, Vec<String>)> =
                serde_json::from_str(assets::book_json(&self.id).expect("book data"))
                    .expect("book json");
            let paragraphs: Vec<String> = chapters.into_iter().flat_map(|(_, p)| p).collect();
            Box::leak(paragraphs.into_boxed_slice())
        })
    }
}

/// The most common words of a language, most frequent first.
pub fn word_list(language: &str) -> &'static [String] {
    static CACHE: OnceLock<Mutex<HashMap<String, &'static [String]>>> = OnceLock::new();
    let mut cache = CACHE
        .get_or_init(Default::default)
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    cache.entry(language.to_string()).or_insert_with(|| {
        let words: Vec<String> = assets::word_list_json(language)
            .and_then(|json| serde_json::from_str(json).ok())
            .unwrap_or_default();
        Box::leak(words.into_boxed_slice())
    })
}

/// Keeps the words made only of the allowed characters.
pub fn filter_word_list(words: &[String], allowed: impl Fn(u32) -> bool) -> Vec<String> {
    words
        .iter()
        .filter(|w| w.chars().all(|c| allowed(u32::from(c))))
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn books() {
        assert_eq!(Book::all().len(), 6);
        let alice = Book::get("en-alice-wonderland");
        assert_eq!(alice.author, "Lewis Carroll");
        assert!(alice.paragraphs()[0].starts_with("Alice was beginning"));
        assert!(alice.paragraphs().len() > 100);
        for book in Book::all() {
            assert!(!book.paragraphs().is_empty(), "{}", book.id);
        }
    }

    #[test]
    fn word_lists() {
        assert_eq!(&word_list("en")[..3], ["the", "to", "i"]);
        assert!(word_list("xx").is_empty());
        let words = filter_word_list(word_list("en"), |cp| {
            "theoa".contains(char::from_u32(cp).unwrap())
        });
        assert!(words.contains(&"the".to_string()));
        assert!(
            !words.contains(&"to".to_string())
                || words.iter().all(|w| w.chars().all(|c| "theoa".contains(c)))
        );
    }
}
