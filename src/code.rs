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
