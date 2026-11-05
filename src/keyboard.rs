//! Keyboard layouts and geometries, ported from `@keybr/keyboard`.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::OnceLock;

use serde::Deserialize;

use crate::assets;
use crate::language::Language;
use crate::unicode::{combine_diacritic, is_diacritic};

pub type KeyId = String;

/// The characters of every key, in the order of the original layout.
pub type KeyCharacters = Vec<(KeyId, Vec<Option<Character>>)>;

/// A character produced by a key.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(untagged)]
pub enum Character {
    CodePoint(u32),
    Dead { dead: u32 },
    Special { special: u32 },
    Ligature { ligature: String },
}

impl Character {
    pub fn code_point(&self) -> Option<u32> {
        match self {
            Character::CodePoint(cp) => Some(*cp),
            _ => None,
        }
    }

    /// A short label to print on a key cap.
    pub fn label(&self) -> String {
        match self {
            Character::CodePoint(cp) => crate::unicode::cp_to_string(*cp),
            Character::Dead { dead } => format!("◌{}", crate::unicode::cp_to_string(*dead)),
            Character::Special { .. } => "·".into(),
            Character::Ligature { ligature } => ligature.clone(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct Layout {
    pub id: String,
    pub xid: u32,
    pub name: String,
    /// Layouts of the same family share their typing statistics.
    pub family: String,
    pub language: String,
    /// Whether the layout can be emulated from a standard US keyboard.
    pub emulate: bool,
    pub geometries: Vec<String>,
    #[serde(rename = "mod")]
    pub layout_mod: Option<String>,
    /// Characters of every key in the original order: unshifted, shifted,
    /// with AltGr, and with Shift and AltGr.
    #[serde(deserialize_with = "deserialize_ordered")]
    pub chars: KeyCharacters,
}

fn deserialize_ordered<'de, D>(d: D) -> Result<KeyCharacters, D::Error>
where
    D: serde::Deserializer<'de>,
{
    struct Ordered;
    impl<'de> serde::de::Visitor<'de> for Ordered {
        type Value = KeyCharacters;
        fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            f.write_str("a map of key characters")
        }
        fn visit_map<A: serde::de::MapAccess<'de>>(
            self,
            mut map: A,
        ) -> Result<Self::Value, A::Error> {
            let mut v = Vec::new();
            while let Some(entry) = map.next_entry()? {
                v.push(entry);
            }
            Ok(v)
        }
    }
    d.deserialize_map(Ordered)
}

impl Layout {
    pub fn all() -> &'static [Layout] {
        static ALL: OnceLock<Vec<Layout>> = OnceLock::new();
        ALL.get_or_init(|| serde_json::from_str(assets::LAYOUTS_JSON).expect("layouts.json"))
    }

    pub fn find(id: &str) -> Option<&'static Layout> {
        Self::all().iter().find(|l| l.id == id)
    }

    /// Returns the layout with the given id, or the US layout.
    pub fn get(id: &str) -> &'static Layout {
        Self::find(id).unwrap_or_else(|| Self::find("en-us").expect("en-us"))
    }

    pub fn language(&self) -> &'static Language {
        Language::get(&self.language)
    }

    /// Layouts which can be used to type the given language, those made for
    /// the language first.
    pub fn selectable(language: &Language) -> Vec<&'static Layout> {
        let same_script: Vec<_> = Self::all()
            .iter()
            .filter(|l| l.language().script == language.script)
            .collect();
        let mut list: Vec<_> = same_script
            .iter()
            .copied()
            .filter(|l| l.language == language.id)
            .collect();
        list.extend(
            same_script
                .iter()
                .copied()
                .filter(|l| l.language != language.id),
        );
        list
    }

    /// The default layout of a language.
    pub fn select(language: &Language) -> &'static Layout {
        Self::selectable(language)
            .first()
            .copied()
            .unwrap_or_else(|| Self::get("en-us"))
    }

    /// The display name including the language, e.g. `English/Colemak`.
    pub fn full_name(&self) -> String {
        format!("{}/{}", self.language().name, self.name)
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct KeyShape {
    pub id: KeyId,
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
    pub labels: Vec<String>,
    pub shape: Option<String>,
    pub zones: Vec<String>,
    pub homing: bool,
}

pub const FINGER_ZONES: &[&str] = &[
    "pinky",
    "ring",
    "middle",
    "leftIndex",
    "rightIndex",
    "thumb",
];
pub const HAND_ZONES: &[&str] = &["left", "right"];
pub const ROW_ZONES: &[&str] = &["digit", "top", "home", "bottom"];

impl KeyShape {
    fn select_zone(&self, zones: &[&'static str]) -> Option<&'static str> {
        zones
            .iter()
            .copied()
            .find(|z| self.zones.iter().any(|s| s == z))
    }

    pub fn finger(&self) -> Option<&'static str> {
        self.select_zone(FINGER_ZONES)
    }

    pub fn hand(&self) -> Option<&'static str> {
        self.select_zone(HAND_ZONES)
    }

    pub fn row(&self) -> Option<&'static str> {
        self.select_zone(ROW_ZONES)
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct Geometry {
    pub id: String,
    pub name: String,
    /// Either `staggered` or `matrix`.
    pub form: String,
    /// Ids of the finger zone variants this geometry supports.
    pub zones: Vec<String>,
    pub keys: Vec<KeyShape>,
}

#[derive(Debug, Clone, Deserialize)]
struct ZoneModData {
    name: String,
    zones: HashMap<KeyId, Vec<String>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GeometriesFile {
    geometries: Vec<Geometry>,
    zone_mods: BTreeMap<String, ZoneModData>,
    layout_mods: HashMap<String, HashMap<KeyId, Vec<String>>>,
}

fn geometries_file() -> &'static GeometriesFile {
    static FILE: OnceLock<GeometriesFile> = OnceLock::new();
    FILE.get_or_init(|| serde_json::from_str(assets::GEOMETRIES_JSON).expect("geometries.json"))
}

impl Geometry {
    pub fn all() -> &'static [Geometry] {
        &geometries_file().geometries
    }

    pub fn get(id: &str) -> &'static Geometry {
        Self::all()
            .iter()
            .find(|g| g.id == id)
            .unwrap_or(&Self::all()[0])
    }
}

/// Finger zone variants (`standard`, `symmetric`) with their display names.
pub fn zone_mods() -> Vec<(&'static str, &'static str)> {
    let mut v: Vec<_> = geometries_file()
        .zone_mods
        .iter()
        .map(|(id, m)| (id.as_str(), m.name.as_str()))
        .collect();
    v.sort_by_key(|(id, _)| if *id == "standard" { 0 } else { 1 });
    v
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Modifier {
    None,
    Shift,
    Alt,
    ShiftAlt,
}

impl Modifier {
    pub fn shift(self) -> bool {
        matches!(self, Modifier::Shift | Modifier::ShiftAlt)
    }

    pub fn alt(self) -> bool {
        matches!(self, Modifier::Alt | Modifier::ShiftAlt)
    }

    fn complexity(self) -> u32 {
        self.shift() as u32 + self.alt() as u32
    }

    const ALL: [Modifier; 4] = [
        Modifier::None,
        Modifier::Shift,
        Modifier::Alt,
        Modifier::ShiftAlt,
    ];
}

/// The key presses required to type a character.
#[derive(Debug, Clone, PartialEq)]
pub struct KeyCombo {
    pub cp: u32,
    pub key: KeyId,
    pub modifier: Modifier,
    /// A dead key to press first.
    pub prefix: Option<Box<KeyCombo>>,
}

impl KeyCombo {
    pub fn complexity(&self) -> u32 {
        let own = 1 + self.modifier.complexity();
        own + self.prefix.as_ref().map_or(0, |p| p.complexity())
    }

    pub fn needs_shift(&self) -> bool {
        self.modifier.shift() || self.prefix.as_ref().is_some_and(|p| p.needs_shift())
    }

    pub fn needs_alt(&self) -> bool {
        self.modifier.alt() || self.prefix.as_ref().is_some_and(|p| p.needs_alt())
    }

    /// The number of modifier keys pressed to type the character.
    pub fn modifier_count(&self) -> u32 {
        self.modifier.complexity() + self.prefix.as_ref().map_or(0, |p| p.modifier_count() + 1)
    }
}

/// A set of typeable code points with a weight for each one,
/// lighter keys are easier to reach.
#[derive(Debug, Clone, Default)]
pub struct CodePointSet {
    pub set: BTreeSet<u32>,
    weights: HashMap<u32, u32>,
}

impl CodePointSet {
    pub fn has(&self, cp: u32) -> bool {
        self.set.contains(&cp)
    }

    /// Home row keys weigh 1, top row keys 2, everything else 1000.
    pub fn weight(&self, cp: u32) -> u32 {
        self.weights.get(&cp).copied().unwrap_or(1000)
    }
}

/// Key combos by code point, in insertion order like a JavaScript `Map`.
#[derive(Default)]
struct ComboMap {
    list: Vec<KeyCombo>,
    index: HashMap<u32, usize>,
}

impl ComboMap {
    fn get(&self, cp: u32) -> Option<&KeyCombo> {
        self.index.get(&cp).map(|&i| &self.list[i])
    }

    /// Adds a combo unless a simpler one already types the same character.
    fn set(&mut self, combo: KeyCombo) {
        match self.index.get(&combo.cp) {
            Some(&i) => {
                if self.list[i].complexity() > combo.complexity() {
                    self.list[i] = combo;
                }
            }
            None => {
                self.index.insert(combo.cp, self.list.len());
                self.list.push(combo);
            }
        }
    }
}

fn remap_zones(shapes: &mut [KeyShape], remap: &HashMap<KeyId, Vec<String>>) {
    for shape in shapes {
        if let Some(zones) = remap.get(&shape.id) {
            shape.zones = zones.clone();
        }
    }
}

fn add_dead_combos(map: &mut ComboMap, dead: u32, key: &str, modifier: Modifier) {
    if !is_diacritic(dead) {
        return;
    }
    let prefix = KeyCombo {
        cp: dead,
        key: key.to_string(),
        modifier,
        prefix: None,
    };
    let bases: Vec<KeyCombo> = map
        .list
        .iter()
        .filter(|c| c.prefix.is_none())
        .cloned()
        .collect();
    for base in bases {
        let combined = combine_diacritic(base.cp, dead);
        if combined != base.cp {
            map.set(KeyCombo {
                cp: combined,
                key: base.key,
                modifier: base.modifier,
                prefix: Some(Box::new(prefix.clone())),
            });
        }
    }
}
