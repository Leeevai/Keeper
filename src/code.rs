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

impl Syntax {
    pub fn all() -> &'static [Syntax] {
        static ALL: OnceLock<Vec<Syntax>> = OnceLock::new();
        ALL.get_or_init(|| {
            let data: Vec<SyntaxData> =
                serde_json::from_str(assets::SYNTAXES_JSON).expect("syntaxes.json");
            data.into_iter()
                .map(|d| {
                    let grammar: Vec<&'static Rules> =
                        d.grammar.iter().map(|n| rule_set(n)).collect();
                    let mut used = HashSet::new();
                    for rules in &grammar {
                        for prod in rules.values() {
                            find_flags(prod, &mut used);
                        }
                    }
                    let flags = FLAGS
                        .iter()
                        .copied()
                        .filter(|f| used.contains(*f))
                        .collect();
                    Syntax {
                        id: d.id,
                        name: d.name,
                        start: d.start,
                        grammar,
                        flags,
                    }
                })
                .collect()
        })
    }

    /// Returns the syntax with the given id, or HTML.
    pub fn get(id: &str) -> &'static Syntax {
        Self::all()
            .iter()
            .find(|s| s.id == id)
            .unwrap_or_else(|| Self::all().iter().find(|s| s.id == "html").unwrap())
    }

    fn rule(&self, name: &str) -> Option<&'static Prod> {
        self.grammar.iter().find_map(|rules| rules.get(name))
    }

    /// Generates about 200 characters of code.
    pub fn generate(&self, flags: &HashSet<String>, rng: &mut dyn Rng) -> StyledText {
        let mut output = Output::new(200);
        let mut generator = Generator {
            syntax: self,
            flags,
            rng,
            cls: Vec::new(),
            last_alt: HashMap::new(),
        };
        let start = self.rule(&self.start).expect("start rule");
        let mut empty_runs = 0;
        loop {
            if output.length() > 0 && output.separate(" ").is_err() {
                break;
            }
            let before = output.length();
            if generator.visit(start, &mut output).is_err() {
                break;
            }
            if output.length() == before {
                empty_runs += 1;
                if empty_runs > 100 {
                    break;
                }
            }
        }
        output.text
    }
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

struct Generator<'a> {
    syntax: &'a Syntax,
    flags: &'a HashSet<String>,
    rng: &'a mut dyn Rng,
    cls: Vec<Class>,
    /// The last choice of every alternative, keyed by its address.
    last_alt: HashMap<usize, usize>,
}

impl Generator<'_> {
    /// Returns whether the production is pruned away by the flags.
    fn pruned(&self, p: &Prod) -> bool {
        match p {
            Prod::Cond { flag, inv, cond } => {
                self.flags.contains(flag) == *inv || self.pruned(cond)
            }
            Prod::Span { span, .. } => self.pruned(span),
            Prod::Opt { opt, .. } => self.pruned(opt),
            Prod::Seq { seq } => seq.iter().all(|c| self.pruned(c)),
            Prod::Alt { alt } => alt.iter().all(|c| self.pruned(c)),
            Prod::Lit(_) | Prod::Ref { .. } => false,
        }
    }

    fn visit(&mut self, p: &Prod, out: &mut Output) -> Result<(), Stop> {
        match p {
            Prod::Cond { flag, inv, cond } => {
                if self.flags.contains(flag) != *inv {
                    self.visit(cond, out)?;
                }
            }
            Prod::Span { cls, span } => {
                if !self.pruned(span) {
                    self.cls.push(Class::parse(cls));
                    let r = self.visit(span, out);
                    self.cls.pop();
                    r?;
                }
            }
            Prod::Opt { f, opt } => {
                if !self.pruned(opt) && (*f >= 1.0 || *f > self.rng.next_f64()) {
                    self.visit(opt, out)?;
                }
            }
            Prod::Seq { seq } => {
                for child in seq {
                    self.visit(child, out)?;
                }
            }
            Prod::Alt { alt } => {
                let choices: Vec<usize> =
                    (0..alt.len()).filter(|&i| !self.pruned(&alt[i])).collect();
                if !choices.is_empty() {
                    let choice = self.choose(alt.as_ptr() as usize, &choices);
                    self.visit(&alt[choice], out)?;
                }
            }
            Prod::Ref { r#ref } => {
                let rule = self
                    .syntax
                    .rule(r#ref)
                    .unwrap_or_else(|| panic!("unknown rule {ref}"));
                self.visit(rule, out)?;
            }
            Prod::Lit(text) => {
                out.append(text, self.cls.last().copied())?;
            }
        }
        Ok(())
    }

    /// Picks a random alternative, never the same one twice in a row.
    fn choose(&mut self, key: usize, choices: &[usize]) -> usize {
        if choices.len() == 1 {
            return choices[0];
        }
        let prev = self.last_alt.get(&key).copied();
        loop {
            let next =
                choices[(self.rng.next_f64() * choices.len() as f64) as usize % choices.len()];
            if Some(next) != prev {
                self.last_alt.insert(key, next);
                return next;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rng::Lcg;
    use crate::textinput::flatten;

    #[test]
    fn every_syntax_generates_code() {
        assert_eq!(Syntax::all().len(), 16);
        let all_flags: HashSet<String> = FLAGS.iter().map(|s| s.to_string()).collect();
        for syntax in Syntax::all() {
            for flags in [HashSet::new(), all_flags.clone()] {
                let mut rng = Lcg::new(11);
                for _ in 0..20 {
                    let text = flatten(&syntax.generate(&flags, &mut rng));
                    let n = text.chars().count();
                    assert!(n > 20 && n <= 200, "{}: {n} {text:?}", syntax.id);
                    assert!(
                        !text.contains('\n') && !text.contains('\t'),
                        "{}",
                        syntax.id
                    );
                }
            }
        }
    }

    #[test]
    fn flags_control_optional_parts() {
        let python = Syntax::get("python");
        assert!(python.flags.contains(&"comments"));
        let comments: HashSet<String> = ["comments".to_string()].into();
        let mut found = false;
        let mut rng = Lcg::new(1);
        for _ in 0..50 {
            let text = python.generate(&comments, &mut rng);
            found |= text.iter().any(|s| s.cls == Some(Class::Comment));
        }
        assert!(found);
        let mut rng = Lcg::new(1);
        for _ in 0..50 {
            let text = python.generate(&HashSet::new(), &mut rng);
            assert!(text.iter().all(|s| s.cls != Some(Class::Comment)));
        }
        assert_eq!(Syntax::get("nope").id, "html");
    }
}
