//! Finds menu items, sizes, quantities, modifiers and conversational cues in
//! normalized tokens, correcting near-miss spellings to real menu items.

use std::collections::HashMap;

use crate::menu::{Menu, Size};
use crate::normalize::normalize;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cue {
    Correction,
    Removal,
    Offer,
    Accept,
    Decline,
    Additive,
    Negation,
    /// "both" / "each": the next size applies to everything just named.
    Every,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MentionKind {
    Item(String),
    Family(String),
    FamilyBase,
    Modifier(String),
    Size(Size),
    Quantity(u32),
    Article,
    Meal,
    Cue(Cue),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Mention {
    pub kind: MentionKind,
    pub start: usize,
    pub end: usize,
    pub score: f32,
    /// Said in plural form ("sauces"), so no number means "unspecified", not one.
    pub plural: bool,
}

/// Conversational phrases. These are English, not menu knowledge.
const CUES: &[(&str, Cue)] = &[
    ("actually", Cue::Correction),
    ("make that", Cue::Correction),
    ("make it", Cue::Correction),
    ("change that", Cue::Correction),
    ("change it", Cue::Correction),
    ("instead", Cue::Correction),
    ("no wait", Cue::Correction),
    ("no weight", Cue::Correction), // how Whisper often hears "no wait"
    ("i mean", Cue::Correction),
    ("never mind", Cue::Removal),
    ("nevermind", Cue::Removal),
    ("take off", Cue::Removal),
    ("remove", Cue::Removal),
    ("cancel", Cue::Removal),
    ("scratch", Cue::Removal),
    ("forget", Cue::Removal),
    ("would you like", Cue::Offer),
    ("do you want", Cue::Offer),
    ("want to try", Cue::Offer),
    ("like to try", Cue::Offer),
    ("can i interest", Cue::Offer),
    ("try our", Cue::Offer),
    ("yes", Cue::Accept),
    ("yeah", Cue::Accept),
    ("yep", Cue::Accept),
    ("sure", Cue::Accept),
    ("sounds good", Cue::Accept),
    ("i'll take", Cue::Accept),
    ("i'll try", Cue::Accept),
    ("let's do", Cue::Accept),
    ("why not", Cue::Accept),
    ("no thanks", Cue::Decline),
    ("no thank you", Cue::Decline),
    ("i'm good", Cue::Decline),
    ("not today", Cue::Decline),
    ("nope", Cue::Decline),
    ("another", Cue::Additive),
    ("one more", Cue::Additive),
    ("also another", Cue::Additive),
    ("plus another", Cue::Additive),
    ("not", Cue::Negation),
    ("both", Cue::Every),
    ("each", Cue::Every),
    ("all of them", Cue::Every),
];

const SIZES: &[(&str, Size)] = &[
    ("small", Size::Sm),
    ("medium", Size::Md),
    ("large", Size::Lg),
    ("big", Size::Lg),
];
const FAMILY_BASE: &[&str] = &["regular", "original", "normal"];
const MEAL: &[&str] = &["meal", "combo"];
const ARTICLES: &[&str] = &["a", "an"];

/// Words that never start or end a fuzzy match ("the chicken" is not an item).
const STOP_WORDS: &[&str] = &[
    "a", "an", "the", "and", "i", "can", "get", "please", "with", "to", "of", "it", "that", "is",
    "you", "me", "for", "my", "some", "have", "will", "be", "do", "on", "in", "your", "our", "so",
];

const MAX_NGRAM: usize = 6;
const MAX_FUZZY_NGRAM: usize = 4;
const FUZZY_MIN_LEN: usize = 5;
const FUZZY_THRESHOLD: f64 = 0.85;
/// A corrected word is never as certain as an exact one, even when the letters line up.
const FUZZY_MAX_SCORE: f64 = 0.95;

/// Lower wins when two tables contain the same phrase.
fn priority(kind: &MentionKind) -> u8 {
    match kind {
        MentionKind::Item(_) => 0,
        MentionKind::Modifier(_) => 1,
        MentionKind::Family(_) => 2,
        _ => 3,
    }
}

pub struct Matcher {
    exact: HashMap<Vec<String>, MentionKind>,
    fuzzy: Vec<FuzzyEntry>,
}

struct FuzzyEntry {
    spaced: String,
    compact: String,
    kind: MentionKind,
}

impl Matcher {
    pub fn new(menu: &Menu) -> Matcher {
        let mut exact: HashMap<Vec<String>, MentionKind> = HashMap::new();
        let mut add = |phrase: &str, kind: MentionKind| {
            let key = normalize(phrase);
            if key.is_empty() {
                return;
            }
            match exact.get(&key) {
                Some(old) if priority(old) <= priority(&kind) => {}
                _ => {
                    exact.insert(key, kind);
                }
            }
        };

        let mut fuzzy = Vec::new();
        for item in &menu.items {
            for phrase in std::iter::once(&item.name).chain(&item.aliases) {
                add(phrase, MentionKind::Item(item.id.clone()));
                push_fuzzy(&mut fuzzy, phrase, MentionKind::Item(item.id.clone()));
            }
        }
        for m in &menu.modifiers {
            for phrase in std::iter::once(&m.name).chain(&m.aliases) {
                add(phrase, MentionKind::Modifier(m.id.clone()));
                push_fuzzy(&mut fuzzy, phrase, MentionKind::Modifier(m.id.clone()));
            }
        }
        for f in &menu.families {
            for phrase in &f.aliases {
                add(phrase, MentionKind::Family(f.id.clone()));
            }
        }
        for (phrase, cue) in CUES {
            add(phrase, MentionKind::Cue(*cue));
        }
        for (phrase, size) in SIZES {
            add(phrase, MentionKind::Size(*size));
        }
        for phrase in FAMILY_BASE {
            add(phrase, MentionKind::FamilyBase);
        }
        for phrase in MEAL {
            add(phrase, MentionKind::Meal);
        }
        for phrase in ARTICLES {
            add(phrase, MentionKind::Article);
        }
        Matcher { exact, fuzzy }
    }

    pub fn find(&self, tokens: &[String]) -> Vec<Mention> {
        let mut out = Vec::new();
        let mut i = 0;
        while i < tokens.len() {
            if let Some(m) = self
                .exact_at(tokens, i)
                .or_else(|| self.fuzzy_at(tokens, i))
            {
                i = m.end;
                out.push(m);
            } else if let Ok(n) = tokens[i].parse::<u32>() {
                out.push(Mention {
                    kind: MentionKind::Quantity(n),
                    start: i,
                    end: i + 1,
                    score: 1.0,
                    plural: false,
                });
                i += 1;
            } else {
                i += 1;
            }
        }
        out
    }

    fn exact_at(&self, tokens: &[String], i: usize) -> Option<Mention> {
        let max = MAX_NGRAM.min(tokens.len() - i);
        for n in (1..=max).rev() {
            let gram = &tokens[i..i + n];
            if let Some(kind) = self
                .exact
                .get(gram)
                .or_else(|| self.exact.get(&singular(gram)?))
            {
                return Some(Mention {
                    kind: kind.clone(),
                    start: i,
                    end: i + n,
                    score: 1.0,
                    plural: is_plural(gram),
                });
            }
        }
        None
    }

    fn fuzzy_at(&self, tokens: &[String], i: usize) -> Option<Mention> {
        let mut best: Option<(f64, usize, &MentionKind)> = None;
        let max = MAX_FUZZY_NGRAM.min(tokens.len() - i);
        for n in 1..=max {
            let gram = &tokens[i..i + n];
            if STOP_WORDS.contains(&gram[0].as_str()) || STOP_WORDS.contains(&gram[n - 1].as_str())
            {
                continue;
            }
            if gram.iter().any(|t| t.chars().all(|c| c.is_ascii_digit())) {
                continue;
            }
            let spaced = gram.join(" ");
            let compact = gram.concat();
            if compact.len() < FUZZY_MIN_LEN {
                continue;
            }
            for e in &self.fuzzy {
                let score = strsim::normalized_damerau_levenshtein(&spaced, &e.spaced)
                    .max(strsim::normalized_damerau_levenshtein(&compact, &e.compact));
                let better = match best {
                    None => true,
                    Some((s, bn, _)) => score > s || (score == s && n > bn),
                };
                if score >= FUZZY_THRESHOLD && better {
                    best = Some((score, n, &e.kind));
                }
            }
        }
        best.map(|(score, n, kind)| Mention {
            kind: kind.clone(),
            start: i,
            end: i + n,
            score: score.min(FUZZY_MAX_SCORE) as f32,
            plural: is_plural(&tokens[i..i + n]),
        })
    }
}

fn push_fuzzy(fuzzy: &mut Vec<FuzzyEntry>, phrase: &str, kind: MentionKind) {
    let tokens = normalize(phrase);
    let compact = tokens.concat();
    if compact.len() >= FUZZY_MIN_LEN {
        fuzzy.push(FuzzyEntry {
            spaced: tokens.join(" "),
            compact,
            kind,
        });
    }
}

/// "sauces", "cookies" — but not "8 count nuggets", which names one item.
fn is_plural(gram: &[String]) -> bool {
    let last = gram.last().map(String::as_str).unwrap_or("");
    last.len() > 3
        && last.ends_with('s')
        && !last.ends_with("ss")
        && !gram.iter().any(|t| t == "count")
}

/// The n-gram with a plural `s` dropped from its last token, if it has one.
fn singular(gram: &[String]) -> Option<Vec<String>> {
    let last = gram.last()?;
    let stem = last.strip_suffix('s').filter(|s| s.len() >= 3)?;
    let mut v = gram.to_vec();
    *v.last_mut()? = stem.to_string();
    Some(v)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::menu::cfa;
    use crate::normalize::normalize;

    fn kinds(text: &str) -> Vec<MentionKind> {
        Matcher::new(&cfa())
            .find(&normalize(text))
            .into_iter()
            .map(|m| m.kind)
            .collect()
    }

    fn no_menu_words(text: &str) -> bool {
        kinds(text).iter().all(|k| {
            !matches!(
                k,
                MentionKind::Item(_) | MentionKind::Family(_) | MentionKind::Modifier(_)
            )
        })
    }

    fn item(id: &str) -> MentionKind {
        MentionKind::Item(id.into())
    }

    #[test]
    fn exact_items_and_quantity() {
        assert_eq!(
            kinds("can i get 3 polynesian sauces"),
            [MentionKind::Quantity(3), item("polynesian")]
        );
    }

    #[test]
    fn longest_match_wins() {
        assert_eq!(kinds("spicy deluxe sandwich"), [item("spicy_deluxe")]);
        assert_eq!(kinds("sweet and spicy sriracha"), [item("sriracha")]);
    }

    #[test]
    fn count_alias() {
        assert_eq!(
            kinds("a twelve count nugget meal"),
            [MentionKind::Article, item("nuggets_12"), MentionKind::Meal]
        );
    }

    #[test]
    fn fuzzy_split_word() {
        let ms = Matcher::new(&cfa()).find(&normalize("polly nesian"));
        assert_eq!(ms.len(), 1);
        assert_eq!(ms[0].kind, item("polynesian"));
        assert!(
            ms[0].score >= 0.85 && ms[0].score < 1.0,
            "score {}",
            ms[0].score
        );
    }

    #[test]
    fn fuzzy_misspelling() {
        assert_eq!(
            kinds("a large lemmonade"),
            [
                MentionKind::Article,
                MentionKind::Size(Size::Lg),
                item("lemonade")
            ]
        );
    }

    #[test]
    fn fuzzy_rejects_far() {
        assert!(no_menu_words("pineapple"));
        assert!(no_menu_words("thank you so much have a great day"));
    }

    #[test]
    fn cues() {
        assert_eq!(
            kinds("actually make that a large"),
            [
                MentionKind::Cue(Cue::Correction),
                MentionKind::Cue(Cue::Correction),
                MentionKind::Article,
                MentionKind::Size(Size::Lg)
            ]
        );
    }

    #[test]
    fn offer() {
        let k = kinds("would you like to try our peach milkshake");
        assert_eq!(k.first(), Some(&MentionKind::Cue(Cue::Offer)));
        assert_eq!(k.last(), Some(&item("peach_shake")));
    }

    #[test]
    fn family_generic() {
        assert_eq!(kinds("nuggets"), [MentionKind::Family("nuggets".into())]);
    }

    #[test]
    fn regular_is_family_base_unless_part_of_item() {
        assert_eq!(
            kinds("regular not spicy"),
            [
                MentionKind::FamilyBase,
                MentionKind::Cue(Cue::Negation),
                item("spicy_sandwich")
            ]
        );
        assert_eq!(kinds("regular lemonade"), [item("lemonade")]);
    }

    #[test]
    fn modifier_beats_decline() {
        assert_eq!(
            kinds("no pickles"),
            [MentionKind::Modifier("no_pickles".into())]
        );
        assert_eq!(kinds("no thanks"), [MentionKind::Cue(Cue::Decline)]);
    }

    #[test]
    fn welcome_is_not_a_sauce() {
        assert!(no_menu_words("welcome to chick fil a"));
    }

    #[test]
    fn additive_after_number_words() {
        assert_eq!(
            kinds("one more cookie"),
            [MentionKind::Cue(Cue::Additive), item("cookie")]
        );
    }

    #[test]
    fn spans_are_token_ranges() {
        let ms = Matcher::new(&cfa()).find(&normalize("3 polynesian sauces"));
        assert_eq!((ms[0].start, ms[0].end), (0, 1));
        assert_eq!((ms[1].start, ms[1].end), (1, 3));
    }
}
