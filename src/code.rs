//! Generates text which resembles source code, ported from `@keybr/code`.

use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;

use serde::Deserialize;

use crate::assets;
use crate::rng::Rng;
use crate::textinput::{Class, Span, StyledText};

/// A production of a grammar.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum Prod {
    Lit(String),
    Cond {
        flag: String,
        inv: bool,
        cond: Box<Prod>,
    },
    Span {
        cls: String,
        span: Box<Prod>,
    },
    Opt {
        f: f64,
        opt: Box<Prod>,
    },
    Seq {
        seq: Vec<Prod>,
    },
    Alt {
        alt: Vec<Prod>,
    },
    Ref {
        r#ref: String,
    },
}

type Rules = HashMap<String, Prod>;

/// Flags which enable optional parts of a grammar.
pub const FLAGS: &[&str] = &[
    "capitals", "comments", "defs", "numbers", "strings", "types",
];

#[derive(Debug, Deserialize)]
struct SyntaxData {
    id: String,
    name: String,
    grammar: Vec<String>,
    start: String,
}

/// A programming language syntax to practice.
pub struct Syntax {
    pub id: String,
    pub name: String,
    start: String,
    /// Rule sets, looked up in order.
    grammar: Vec<&'static Rules>,
    /// The flags used by the grammar.
    pub flags: Vec<&'static str>,
}

fn rule_set(name: &str) -> &'static Rules {
    static CACHE: OnceLock<HashMap<String, Rules>> = OnceLock::new();
    let all = CACHE.get_or_init(|| {
        [
            "_comments",
            "_ident",
            "_keywords",
            "_literals",
            "lang_cpp",
            "lang_csharp",
            "lang_go",
            "lang_html_css",
            "lang_java",
            "lang_javascript",
            "lang_php",
            "lang_python",
            "lang_regex",
            "lang_rust",
            "lang_shell",
            "lang_typescript",
        ]
        .into_iter()
        .map(|n| {
            (
                n.to_string(),
                serde_json::from_str(assets::grammar_json(n).unwrap()).expect(n),
            )
        })
        .collect()
    });
    &all[name]
}

fn find_flags(prod: &Prod, flags: &mut HashSet<String>) {
    match prod {
        Prod::Cond { flag, cond, .. } => {
            flags.insert(flag.clone());
            find_flags(cond, flags);
        }
        Prod::Span { span, .. } => find_flags(span, flags),
        Prod::Opt { opt, .. } => find_flags(opt, flags),
        Prod::Seq { seq } => seq.iter().for_each(|p| find_flags(p, flags)),
        Prod::Alt { alt } => alt.iter().for_each(|p| find_flags(p, flags)),
        Prod::Lit(_) | Prod::Ref { .. } => {}
    }
}

/// The output buffer stops generation when its limit is reached.
struct Output {
    limit: usize,
    text: StyledText,
    length: usize,
    separator: String,
}

struct Stop;

impl Output {
    fn new(limit: usize) -> Self {
        Self {
            limit,
            text: Vec::new(),
            length: 0,
            separator: String::new(),
        }
    }

    fn length(&self) -> usize {
        self.length + self.separator.chars().count()
    }

    fn separate(&mut self, text: &str) -> Result<(), Stop> {
        if !self.text.is_empty() {
            let n = text.chars().count();
            if n > 0 && self.length + n > self.limit {
                return Err(Stop);
            }
            self.separator = text.to_string();
        }
        Ok(())
    }

    fn append(&mut self, text: &str, cls: Option<Class>) -> Result<(), Stop> {
        if (text == " " || text == "\n" || text == "\t") && cls.is_none() {
            return self.separate(text);
        }
        let n = text.chars().count();
        if n > 0 {
            if self.length + self.separator.chars().count() + n > self.limit {
                return Err(Stop);
            }
            if !self.separator.is_empty() {
                let sep = std::mem::take(&mut self.separator);
                self.length += sep.chars().count();
                // Line breaks and tabs are typed as spaces.
                self.text.push(Span {
                    text: " ".repeat(sep.chars().count()),
                    cls: None,
                });
            }
            self.text.push(Span {
                text: text.to_string(),
                cls,
            });
            self.length += n;
        }
        Ok(())
    }
}
