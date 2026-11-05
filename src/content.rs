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
