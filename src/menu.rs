//! The menu file: everything Order Check knows about one restaurant.

use std::collections::{HashMap, HashSet};
use std::path::Path;

use anyhow::{Context, bail};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Category {
    Entree,
    Side,
    Sauce,
    Drink,
    Dessert,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Size {
    Sm,
    Md,
    Lg,
}

impl Size {
    pub fn word(&self) -> &'static str {
        match self {
            Size::Sm => "Small",
            Size::Md => "Medium",
            Size::Lg => "Large",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MenuItem {
    pub id: String,
    pub name: String,
    pub pos: String,
    pub category: Category,
    pub price: f32,
    #[serde(default)]
    pub sized: bool,
    pub family: String,
    #[serde(default)]
    pub meal: bool,
    #[serde(default)]
    pub aliases: Vec<String>,
    #[serde(default)]
    pub modifiers: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Family {
    pub id: String,
    pub base: String,
    #[serde(default)]
    pub aliases: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Modifier {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub aliases: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MealRule {
    pub default_side: String,
    pub default_size: Size,
    pub upcharge: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StandardKind {
    Phrase,
    OfferLto,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StandardDef {
    pub id: String,
    pub kind: StandardKind,
    #[serde(default)]
    pub phrases: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Menu {
    pub restaurant: String,
    pub items: Vec<MenuItem>,
    #[serde(default)]
    pub families: Vec<Family>,
    #[serde(default)]
    pub modifiers: Vec<Modifier>,
    pub meal: MealRule,
    #[serde(default)]
    pub lto_items: Vec<String>,
    #[serde(default)]
    pub standards: Vec<StandardDef>,
    #[serde(skip)]
    index: Index,
}

#[derive(Debug, Clone, Default)]
struct Index {
    items: HashMap<String, usize>,
    families: HashMap<String, usize>,
    modifiers: HashMap<String, usize>,
}

/// Longest vocabulary prompt handed to Whisper (it only reads ~224 tokens).
const PROMPT_LIMIT: usize = 900;

impl Menu {
    pub fn load(path: &Path) -> anyhow::Result<Menu> {
        let text = std::fs::read_to_string(path)
            .with_context(|| format!("reading menu file {}", path.display()))?;
        Menu::from_toml_str(&text).with_context(|| format!("in menu file {}", path.display()))
    }

    pub fn from_toml_str(s: &str) -> anyhow::Result<Menu> {
        let mut menu: Menu = toml::from_str(s)?;
        menu.index = Index {
            items: index_ids(menu.items.iter().map(|i| i.id.as_str()), "item")?,
            families: index_ids(menu.families.iter().map(|f| f.id.as_str()), "family")?,
            modifiers: index_ids(menu.modifiers.iter().map(|m| m.id.as_str()), "modifier")?,
        };
        menu.validate()?;
        Ok(menu)
    }

    fn validate(&self) -> anyhow::Result<()> {
        for item in &self.items {
            for m in &item.modifiers {
                if self.modifier(m).is_none() {
                    bail!("item {} lists unknown modifier {m}", item.id);
                }
            }
        }
        for f in &self.families {
            if self.item(&f.base).is_none() {
                bail!("family {} has unknown base item {}", f.id, f.base);
            }
        }
        for id in &self.lto_items {
            if self.item(id).is_none() {
                bail!("lto_items lists unknown item {id}");
            }
        }
        if self.item(&self.meal.default_side).is_none() {
            bail!(
                "meal default_side is unknown item {}",
                self.meal.default_side
            );
        }
        Ok(())
    }

    pub fn item(&self, id: &str) -> Option<&MenuItem> {
        self.index.items.get(id).map(|&i| &self.items[i])
    }

    pub fn family(&self, id: &str) -> Option<&Family> {
        self.index.families.get(id).map(|&i| &self.families[i])
    }

    pub fn modifier(&self, id: &str) -> Option<&Modifier> {
        self.index.modifiers.get(id).map(|&i| &self.modifiers[i])
    }

    /// Menu words to bias Whisper toward, most important (items) first.
    pub fn vocabulary_prompt(&self) -> String {
        let mut seen = HashSet::new();
        let mut prompt = String::new();
        let names = self
            .items
            .iter()
            .map(|i| &i.name)
            .chain(self.modifiers.iter().map(|m| &m.name));
        for name in names {
            if !seen.insert(name.as_str()) {
                continue;
            }
            let sep = if prompt.is_empty() { "" } else { ", " };
            if prompt.len() + sep.len() + name.len() > PROMPT_LIMIT {
                break;
            }
            prompt.push_str(sep);
            prompt.push_str(name);
        }
        prompt
    }
}

fn index_ids<'a>(
    ids: impl Iterator<Item = &'a str>,
    what: &str,
) -> anyhow::Result<HashMap<String, usize>> {
    let mut map = HashMap::new();
    for (i, id) in ids.enumerate() {
        if map.insert(id.to_string(), i).is_some() {
            bail!("duplicate {what} id {id}");
        }
    }
    Ok(map)
}

#[cfg(test)]
pub(crate) fn cfa() -> Menu {
    Menu::load(Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/menus/chick-fil-a.toml"
    )))
    .unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;

    const SMALL: &str = r#"
        restaurant = "Test"
        lto_items = []
        [meal]
        default_side = "fries"
        default_size = "md"
        upcharge = 1.0
        [[items]]
        id = "fries"
        name = "Fries"
        pos = "Fries"
        category = "side"
        price = 2.0
        family = "fries"
        modifiers = ["no_salt"]
        [[modifiers]]
        id = "no_salt"
        name = "No Salt"
    "#;

    #[test]
    fn loads_cfa_menu() {
        let m = cfa();
        assert_eq!(m.item("spicy_sandwich").unwrap().family, "sandwich");
        assert_eq!(m.family("nuggets").unwrap().base, "nuggets_8");
        assert_eq!(m.item("polynesian").unwrap().price, 0.0);
        assert_eq!(m.modifier("no_pickles").unwrap().name, "No Pickles");
        assert_eq!(Size::Lg.word(), "Large");
    }

    #[test]
    fn generic_menu_loads() {
        let m = Menu::load(Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/menus/generic-grill.toml"))).unwrap();
        assert_eq!(m.item("bacon_cheeseburger").unwrap().family, "burger");
        assert_eq!(m.family("burger").unwrap().base, "hamburger");
    }

    #[test]
    fn small_menu_is_valid() {
        assert!(Menu::from_toml_str(SMALL).is_ok());
    }

    fn err_of(toml: &str) -> String {
        format!("{:#}", Menu::from_toml_str(toml).unwrap_err())
    }

    #[test]
    fn validate_rejects_unknown_refs() {
        let bad_mod = SMALL.replace(
            r#"modifiers = ["no_salt"]"#,
            r#"modifiers = ["no_such_mod"]"#,
        );
        assert!(err_of(&bad_mod).contains("no_such_mod"));

        let bad_lto = SMALL.replace("lto_items = []", r#"lto_items = ["ghost_shake"]"#);
        assert!(err_of(&bad_lto).contains("ghost_shake"));

        let bad_side = SMALL.replace(
            r#"default_side = "fries""#,
            r#"default_side = "onion_rings""#,
        );
        assert!(err_of(&bad_side).contains("onion_rings"));

        let bad_family = format!("{SMALL}\n[[families]]\nid = \"fries\"\nbase = \"curly\"\n");
        assert!(err_of(&bad_family).contains("curly"));

        let dup = format!(
            "{SMALL}\n[[items]]\nid = \"fries\"\nname = \"F2\"\npos = \"F2\"\ncategory = \"side\"\nprice = 1.0\nfamily = \"fries\"\n"
        );
        assert!(err_of(&dup).contains("duplicate"));
    }

    #[test]
    fn vocabulary_prompt_contains_menu_terms() {
        let p = cfa().vocabulary_prompt();
        assert!(p.contains("Polynesian"), "{p}");
        assert!(p.contains("Chick-fil-A Sauce"), "{p}");
        assert!(p.contains("Spicy Deluxe"), "{p}");
        assert!(p.len() <= 900, "prompt is {} chars", p.len());
    }
}
