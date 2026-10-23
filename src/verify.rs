//! Compares the spoken order with the ticket and explains every difference.

use serde::{Deserialize, Serialize};

use crate::extract::{SpokenComponent, SpokenOrder};
use crate::menu::{Menu, Size};
use crate::ticket::{Ticket, TicketComponent};

/// Below this, a mismatch is reported as "couldn't confirm" instead of buzzing.
/// Whisper's token probabilities run lower on short phrases, so this sits at 0.5.
pub const MIN_CONFIDENCE: f32 = 0.5;

/// Not hearing something is weaker evidence than hearing something else, so
/// "rang X, guest didn't ask" (and a meal or modifier that wasn't heard) only
/// buzzes when the whole transcript came through clearly. Tuned on the eval
/// set: 0.75 keeps false alarms on clean audio under 5%.
pub const ABSENCE_MIN_CONFIDENCE: f32 = 0.75;

fn absence_conf(transcript_confidence: f32) -> f32 {
    if transcript_confidence >= ABSENCE_MIN_CONFIDENCE {
        transcript_confidence
    } else {
        0.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MismatchKind {
    Variant,
    Count,
    Size,
    Modifier,
    Missing,
    Extra,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Mismatch {
    pub kind: MismatchKind,
    pub message: String,
    pub said: Option<String>,
    pub rang: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    Match,
    Mismatch,
    Unconfirmed,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CheckResult {
    pub outcome: Outcome,
    pub mismatches: Vec<Mismatch>,
    pub unconfirmed: Vec<Mismatch>,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Role {
    Entree { meal: bool },
    Component,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MealPart {
    Side,
    Drink,
}

/// One comparable thing on either side: an entrée, a side, a drink, a sauce.
#[derive(Debug, Clone)]
struct Unit {
    item: Option<String>,
    family: String,
    qty: Option<u32>,
    /// The quantity was said out loud rather than implied by a bare item name.
    qty_said: bool,
    size: Option<Size>,
    modifiers: Vec<String>,
    role: Role,
    part: Option<MealPart>,
    default_side: bool,
    confidence: f32,
}

impl Unit {
    fn key(&self) -> (Option<String>, String, Option<Size>, Vec<String>, bool) {
        let meal = matches!(self.role, Role::Entree { meal: true });
        (
            self.item.clone(),
            self.family.clone(),
            self.size,
            self.modifiers.clone(),
            meal,
        )
    }
}

fn spoken_units(menu: &Menu, spoken: &SpokenOrder) -> Vec<Unit> {
    let mut out = Vec::new();
    for l in &spoken.lines {
        let entree = l
            .item
            .as_deref()
            .and_then(|id| menu.item(id))
            .or_else(|| menu.family(&l.family).and_then(|f| menu.item(&f.base)))
            .is_some_and(|i| i.category == crate::menu::Category::Entree);
        let role = if entree {
            Role::Entree {
                meal: l.meal.is_some(),
            }
        } else {
            Role::Component
        };
        out.push(Unit {
            item: l.item.clone(),
            family: l.family.clone(),
            qty: l.qty,
            qty_said: l.qty_said,
            size: l.size,
            modifiers: sorted(&l.modifiers),
            role,
            part: None,
            default_side: false,
            confidence: l.confidence,
        });
        if let Some(meal) = &l.meal {
            for (part, c) in [(MealPart::Side, &meal.side), (MealPart::Drink, &meal.drink)] {
                if let Some(c) = c {
                    let mut u = spoken_component(c, l.qty, l.meal_size, part);
                    u.qty_said = l.qty_said;
                    out.push(u);
                }
            }
        }
    }
    out
}

fn spoken_component(
    c: &SpokenComponent,
    qty: Option<u32>,
    meal_size: Option<Size>,
    part: MealPart,
) -> Unit {
    Unit {
        item: c.item.clone(),
        family: c.family.clone(),
        qty,
        qty_said: false,
        size: c.size.or(meal_size),
        modifiers: sorted(&c.modifiers),
        role: Role::Component,
        part: Some(part),
        default_side: false,
        confidence: c.confidence,
    }
}

fn ticket_units(menu: &Menu, ticket: &Ticket) -> Vec<Unit> {
    let family = |id: &str| menu.item(id).map(|i| i.family.clone()).unwrap_or_default();
    let mut out = Vec::new();
    for l in &ticket.lines {
        let entree = menu
            .item(&l.item)
            .is_some_and(|i| i.category == crate::menu::Category::Entree);
        out.push(Unit {
            item: Some(l.item.clone()),
            family: family(&l.item),
            qty: Some(l.qty),
            qty_said: true,
            size: l.size,
            modifiers: sorted(&l.modifiers),
            role: if entree {
                Role::Entree {
                    meal: l.meal.is_some(),
                }
            } else {
                Role::Component
            },
            part: None,
            default_side: false,
            confidence: 1.0,
        });
        if let Some(meal) = &l.meal {
            let mut push = |c: &TicketComponent, part: MealPart| {
                let default_side = part == MealPart::Side
                    && c.item == menu.meal.default_side
                    && c.size.is_none_or(|s| s == menu.meal.default_size)
                    && c.modifiers.is_empty();
                out.push(Unit {
                    item: Some(c.item.clone()),
                    family: family(&c.item),
                    qty: Some(l.qty),
                    qty_said: true,
                    size: c.size,
                    modifiers: sorted(&c.modifiers),
                    role: Role::Component,
                    part: Some(part),
                    default_side,
                    confidence: 1.0,
                });
            };
            push(&meal.side, MealPart::Side);
            if let Some(d) = &meal.drink {
                push(d, MealPart::Drink);
            }
        }
    }
    out
}

fn sorted(v: &[String]) -> Vec<String> {
    let mut v = v.to_vec();
    v.sort();
    v
}

/// Sums quantities of identical units. An unspecified spoken quantity stays unspecified.
fn aggregate(units: Vec<Unit>) -> Vec<Unit> {
    let mut out: Vec<Unit> = Vec::new();
    for u in units {
        if let Some(existing) = out
            .iter_mut()
            .find(|e| e.key() == u.key() && e.role == u.role)
        {
            existing.qty = match (existing.qty, u.qty) {
                (Some(a), Some(b)) => Some(a + b),
                _ => None,
            };
            existing.qty_said &= u.qty_said;
            existing.confidence = existing.confidence.min(u.confidence);
            existing.default_side &= u.default_side;
            existing.part = existing.part.or(u.part);
        } else {
            out.push(u);
        }
    }
    out
}

struct Namer<'a> {
    menu: &'a Menu,
}

impl Namer<'_> {
    fn base(&self, u: &Unit) -> String {
        let name = match &u.item {
            Some(id) => self
                .menu
                .item(id)
                .map(|i| i.name.clone())
                .unwrap_or_else(|| id.clone()),
            None => title_case(&u.family),
        };
        if matches!(u.role, Role::Entree { meal: true }) {
            format!("{name} Meal")
        } else {
            name
        }
    }

    fn sized(&self, u: &Unit, size: Option<Size>) -> String {
        let sized_item = u
            .item
            .as_deref()
            .and_then(|id| self.menu.item(id))
            .is_some_and(|i| i.sized);
        match size {
            Some(s) if sized_item => format!("{} {}", s.word(), self.base(u)),
            _ => self.base(u),
        }
    }

    fn modifier(&self, id: &str) -> String {
        self.menu
            .modifier(id)
            .map(|m| m.name.clone())
            .unwrap_or_else(|| id.to_string())
    }
}

fn title_case(id: &str) -> String {
    id.split('_')
        .map(|w| {
            let mut c = w.chars();
            c.next()
                .map(|f| f.to_uppercase().chain(c).collect::<String>())
                .unwrap_or_default()
        })
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn verify(
    menu: &Menu,
    spoken: &SpokenOrder,
    ticket: &Ticket,
    transcript_confidence: f32,
) -> CheckResult {
    let namer = Namer { menu };
    let said = aggregate(spoken_units(menu, spoken));
    let rang = aggregate(ticket_units(menu, ticket));
    let unspecified_drinks = spoken
        .lines
        .iter()
        .any(|l| l.meal.as_ref().is_some_and(|m| m.drink.is_none()));
    let unspecified_sides = spoken
        .lines
        .iter()
        .any(|l| l.meal.as_ref().is_some_and(|m| m.side.is_none()));

    let mut found: Vec<(Mismatch, f32)> = Vec::new();
    let mut said_used = vec![false; said.len()];
    let mut rang_used = vec![false; rang.len()];

    // Pass A: same item. Pass B: same family (a look-alike was rung).
    for pass_family in [false, true] {
        for (si, s) in said.iter().enumerate() {
            if said_used[si] || (!pass_family && s.item.is_none()) {
                continue;
            }
            let fits = |r: &Unit| {
                if pass_family {
                    r.family == s.family
                } else {
                    r.item == s.item
                }
            };
            let candidates: Vec<usize> = (0..rang.len())
                .filter(|&ri| !rang_used[ri] && fits(&rang[ri]))
                .collect();
            let best = candidates.iter().copied().max_by_key(|&ri| {
                let r = &rang[ri];
                (
                    r.role == s.role,
                    s.size.is_none() || r.size == s.size,
                    r.modifiers == s.modifiers,
                    r.part == s.part,
                )
            });
            let Some(ri) = best else { continue };
            said_used[si] = true;
            rang_used[ri] = true;
            let mut s = s.clone();
            let mut r = rang[ri].clone();
            // "a diet lemonade and a lemonade" rung as two lemonades: the
            // extra count is really a look-alike, so report it as one.
            if let (false, Some(a), Some(b)) = (pass_family, s.qty, r.qty) {
                if b > a {
                    let mate = (0..said.len()).find(|&j| {
                        let m = &said[j];
                        !said_used[j]
                            && m.family == s.family
                            && m.item.is_some()
                            && m.item != s.item
                            && m.qty.is_none_or(|q| q == b - a)
                            // only if nothing on the ticket is that item already
                            && !(0..rang.len()).any(|k| !rang_used[k] && rang[k].item == m.item)
                    });
                    if let Some(j) = mate {
                        said_used[j] = true;
                        found.push((lookalike(&namer, &said[j], &r), said[j].confidence));
                        r.qty = Some(a);
                    }
                } else if a > b {
                    let mate = (0..rang.len()).find(|&j| {
                        let m = &rang[j];
                        !rang_used[j]
                            && m.family == s.family
                            && m.item != s.item
                            && m.qty == Some(a - b)
                            && !(0..said.len()).any(|k| !said_used[k] && said[k].item == m.item)
                    });
                    if let Some(j) = mate {
                        rang_used[j] = true;
                        found.push((lookalike(&namer, &s, &rang[j]), s.confidence));
                        s.qty = Some(b);
                    }
                }
            }
            compare_pair(
                &namer,
                &s,
                &r,
                pass_family,
                transcript_confidence,
                &mut found,
            );
        }
    }

    for (si, s) in said.iter().enumerate() {
        if !said_used[si] {
            let qty = s.qty.map(|q| format!("{q} ")).unwrap_or_default();
            let message = format!("Said {qty}{}, not on ticket", namer.sized(s, s.size));
            found.push((
                Mismatch {
                    kind: MismatchKind::Missing,
                    message,
                    said: s.item.clone(),
                    rang: None,
                },
                s.confidence,
            ));
        }
    }
    let nothing_heard = spoken.lines.is_empty();
    let extra_confidence = if nothing_heard {
        0.0
    } else {
        absence_conf(transcript_confidence)
    };
    let excused = |r: &Unit| {
        r.default_side
            || match r.part {
                Some(MealPart::Drink) => unspecified_drinks,
                Some(MealPart::Side) => unspecified_sides,
                None => false,
            }
    };
    let considered = rang.iter().filter(|r| !excused(r)).count();
    let extras: Vec<usize> = (0..rang.len())
        .filter(|&ri| !rang_used[ri] && !excused(&rang[ri]))
        .collect();
    // Hearing only part of the order (a clipped read-back, a passing truck)
    // is not evidence that the rest was never said.
    let heard_only_part = !nothing_heard && extras.len() >= 2 && extras.len() * 2 > considered;
    let extra_confidence = if heard_only_part {
        0.0
    } else {
        extra_confidence
    };
    for &ri in &extras {
        let r = &rang[ri];
        let message = format!(
            "Rang {} {}, guest didn't ask",
            r.qty.unwrap_or(1),
            namer.sized(r, r.size)
        );
        found.push((
            Mismatch {
                kind: MismatchKind::Extra,
                message,
                said: None,
                rang: r.item.clone(),
            },
            extra_confidence,
        ));
    }

    let mut result = CheckResult {
        outcome: Outcome::Match,
        mismatches: Vec::new(),
        unconfirmed: Vec::new(),
        reason: None,
    };
    for (m, conf) in found {
        if conf < MIN_CONFIDENCE {
            result.unconfirmed.push(m)
        } else {
            result.mismatches.push(m)
        }
    }
    if nothing_heard && !ticket.lines.is_empty() {
        result.reason = Some("nothing heard".into());
    } else if heard_only_part {
        result.reason = Some("heard only part of the order".into());
    }
    result.outcome = if !result.mismatches.is_empty() {
        Outcome::Mismatch
    } else if !result.unconfirmed.is_empty() || result.reason.is_some() {
        Outcome::Unconfirmed
    } else {
        Outcome::Match
    };
    result
}

fn lookalike(namer: &Namer, s: &Unit, r: &Unit) -> Mismatch {
    Mismatch {
        kind: MismatchKind::Variant,
        message: format!(
            "Said {}, rang {}",
            namer.base(s).to_uppercase(),
            namer.base(r).to_uppercase()
        ),
        said: s.item.clone(),
        rang: r.item.clone(),
    }
}

fn compare_pair(
    namer: &Namer,
    s: &Unit,
    r: &Unit,
    same_family_only: bool,
    transcript_confidence: f32,
    found: &mut Vec<(Mismatch, f32)>,
) {
    let heard = s.confidence;
    let unheard = s.confidence.min(absence_conf(transcript_confidence));
    let mut push = |kind, message: String, conf: f32| {
        found.push((
            Mismatch {
                kind,
                message,
                said: s.item.clone(),
                rang: r.item.clone(),
            },
            conf,
        ));
    };
    let variant = (same_family_only && s.item.is_some()) || s.role != r.role;
    if variant {
        // Not hearing "meal" is weaker evidence than hearing a different item.
        let meal_unheard = !same_family_only
            && s.role == Role::Entree { meal: false }
            && r.role == Role::Entree { meal: true };
        push(
            MismatchKind::Variant,
            format!(
                "Said {}, rang {}",
                namer.base(s).to_uppercase(),
                namer.base(r).to_uppercase()
            ),
            if meal_unheard { unheard } else { heard },
        );
    }
    if let (Some(a), Some(b)) = (s.qty, r.qty)
        && a != b
    {
        // "polynesian sauce" with no number implies one, but only weakly.
        push(
            MismatchKind::Count,
            format!("Said {a} {}, rang {b}", namer.base(s)),
            if s.qty_said { heard } else { unheard },
        );
    }
    let sized = s
        .item
        .as_deref()
        .or(r.item.as_deref())
        .and_then(|id| namer.menu.item(id))
        .is_some_and(|i| i.sized);
    if let Some(ss) = s.size
        && sized
        && Some(ss) != r.size
    {
        let rang_word = r.size.map(|x| x.word()).unwrap_or("no size");
        push(
            MismatchKind::Size,
            format!("Said {} {}, rang {rang_word}", ss.word(), namer.base(s)),
            heard,
        );
    }
    let item_name = namer.base(r);
    for m in s.modifiers.iter().filter(|m| !r.modifiers.contains(m)) {
        push(
            MismatchKind::Modifier,
            format!("Said {} on {item_name}, not on ticket", namer.modifier(m)),
            heard,
        );
    }
    for m in r.modifiers.iter().filter(|m| !s.modifiers.contains(m)) {
        push(
            MismatchKind::Modifier,
            format!(
                "Rang {} on {item_name}, guest didn't ask",
                namer.modifier(m)
            ),
            unheard,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::extract::{extract, utts};
    use crate::matcher::Matcher;
    use crate::menu::cfa;

    fn check_at(said: &[&str], conf: f32, ticket: serde_json::Value) -> CheckResult {
        let menu = cfa();
        let spoken = extract(&menu, &Matcher::new(&menu), &utts(said, conf));
        let ticket: Ticket = serde_json::from_value(ticket).unwrap();
        ticket.validate(&menu).unwrap();
        let tc = if said.is_empty() { 0.0 } else { conf };
        verify(&menu, &spoken, &ticket, tc)
    }

    fn check(said: &[&str], ticket: serde_json::Value) -> CheckResult {
        check_at(said, 0.95, ticket)
    }

    fn messages(r: &CheckResult) -> Vec<&str> {
        let mut m: Vec<&str> = r.mismatches.iter().map(|m| m.message.as_str()).collect();
        m.sort();
        m
    }

    fn meal(item: &str, drink: &str) -> serde_json::Value {
        serde_json::json!({"item": item, "qty": 1, "meal": {
            "side": {"item": "waffle_fries", "size": "md"},
            "drink": {"item": drink, "size": "md"}}})
    }

    const SPICY: &[&str] = &[
        "Can I get a spicy chicken sandwich meal with a medium sweet tea, and three Polynesian sauces?",
    ];

    #[test]
    fn match_exact() {
        let r = check(
            SPICY,
            serde_json::json!({"lines": [meal("spicy_sandwich", "sweet_tea"), {"item": "polynesian", "qty": 3}]}),
        );
        assert_eq!(r.outcome, Outcome::Match, "{r:#?}");
        assert!(r.mismatches.is_empty() && r.unconfirmed.is_empty());
    }

    #[test]
    fn variant_and_count() {
        let r = check(
            SPICY,
            serde_json::json!({"lines": [meal("chicken_sandwich", "sweet_tea"), {"item": "polynesian", "qty": 1}]}),
        );
        assert_eq!(r.outcome, Outcome::Mismatch);
        assert_eq!(
            messages(&r),
            [
                "Said 3 Polynesian, rang 1",
                "Said SPICY CHICKEN SANDWICH MEAL, rang CHICKEN SANDWICH MEAL"
            ]
        );
        let variant = r
            .mismatches
            .iter()
            .find(|m| m.kind == MismatchKind::Variant)
            .unwrap();
        assert_eq!(
            (variant.said.as_deref(), variant.rang.as_deref()),
            (Some("spicy_sandwich"), Some("chicken_sandwich"))
        );
    }

    #[test]
    fn size() {
        let r = check(
            &["a large lemonade"],
            serde_json::json!({"lines": [{"item": "lemonade", "qty": 1, "size": "md"}]}),
        );
        assert_eq!(messages(&r), ["Said Large Lemonade, rang Medium"]);
    }

    #[test]
    fn modifier_missing_and_extra() {
        let r = check(
            &["a chicken sandwich no pickles"],
            serde_json::json!({"lines": [{"item": "chicken_sandwich", "qty": 1, "modifiers": ["add_cheese"]}]}),
        );
        assert_eq!(
            messages(&r),
            [
                "Rang Add Cheese on Chicken Sandwich, guest didn't ask",
                "Said No Pickles on Chicken Sandwich, not on ticket"
            ]
        );
    }

    #[test]
    fn missing_and_extra() {
        let r = check(
            &["two chick fil a sauces"],
            serde_json::json!({"lines": [{"item": "cookie", "qty": 1}]}),
        );
        assert_eq!(
            messages(&r),
            [
                "Rang 1 Cookie, guest didn't ask",
                "Said 2 Chick-fil-A Sauce, not on ticket"
            ]
        );
    }

    #[test]
    fn unspecified_is_not_mismatch() {
        let r = check(
            &["some nuggets and a lemonade"],
            serde_json::json!({"lines": [{"item": "nuggets_12", "qty": 1}, {"item": "lemonade", "qty": 1, "size": "lg"}]}),
        );
        assert_eq!(r.outcome, Outcome::Match, "{r:#?}");
    }

    #[test]
    fn meal_drink_unspecified() {
        let r = check(
            &["a spicy chicken sandwich meal"],
            serde_json::json!({"lines": [meal("spicy_sandwich", "sweet_tea")]}),
        );
        assert_eq!(r.outcome, Outcome::Match, "{r:#?}");
    }

    #[test]
    fn meal_vs_no_meal_is_variant() {
        let r = check(
            &["a spicy chicken sandwich meal"],
            serde_json::json!({"lines": [{"item": "spicy_sandwich", "qty": 1}]}),
        );
        assert_eq!(
            messages(&r),
            ["Said SPICY CHICKEN SANDWICH MEAL, rang SPICY CHICKEN SANDWICH"]
        );
    }

    #[test]
    fn large_meal_sizes_its_drink() {
        let r = check(
            &["a large spicy chicken sandwich meal with a lemonade"],
            serde_json::json!({"lines": [meal("spicy_sandwich", "lemonade")]}),
        );
        assert_eq!(messages(&r), ["Said Large Lemonade, rang Medium"]);
    }

    #[test]
    fn sauces_split_across_lines_are_summed() {
        let r = check(
            &["three polynesian"],
            serde_json::json!({"lines": [{"item": "polynesian", "qty": 1}, {"item": "polynesian", "qty": 2}]}),
        );
        assert_eq!(r.outcome, Outcome::Match, "{r:#?}");
    }

    #[test]
    fn aggregated_lookalike_is_variant() {
        let r = check(
            &["a medium diet lemonade and a medium regular lemonade"],
            serde_json::json!({"lines": [{"item": "lemonade", "qty": 1, "size": "md"}, {"item": "lemonade", "qty": 1, "size": "md"}]}),
        );
        assert_eq!(messages(&r), ["Said DIET LEMONADE, rang LEMONADE"]);
    }

    #[test]
    fn count_is_not_mistaken_for_lookalike() {
        let r = check(
            &["a medium diet lemonade and a medium regular lemonade"],
            serde_json::json!({"lines": [{"item": "diet_lemonade", "qty": 2, "size": "md"}, {"item": "lemonade", "qty": 1, "size": "md"}]}),
        );
        assert_eq!(messages(&r), ["Said 1 Diet Lemonade, rang 2"]);
    }

    #[test]
    fn meal_not_heard_is_unconfirmed_unless_clear() {
        let ticket = serde_json::json!({"lines": [meal("spicy_sandwich", "sweet_tea")]});
        let muddy = check_at(&["a spicy chicken sandwich"], 0.7, ticket.clone());
        assert_eq!(muddy.outcome, Outcome::Unconfirmed, "{muddy:#?}");
        let clear = check_at(&["a spicy chicken sandwich"], 0.95, ticket);
        assert_eq!(clear.outcome, Outcome::Mismatch, "{clear:#?}");
    }

    #[test]
    fn extra_item_needs_a_clear_transcript() {
        let ticket = serde_json::json!({"lines": [{"item": "lemonade", "qty": 1, "size": "lg"}, {"item": "cookie", "qty": 1}]});
        assert_eq!(
            check_at(&["a large lemonade"], 0.7, ticket.clone()).outcome,
            Outcome::Unconfirmed
        );
        assert_eq!(
            check_at(&["a large lemonade"], 0.95, ticket).outcome,
            Outcome::Mismatch
        );
    }

    #[test]
    fn partial_readback_is_unconfirmed() {
        let r = check(
            &["a large lemonade, got it"],
            serde_json::json!({"lines": [{"item": "lemonade", "qty": 1, "size": "lg"}, {"item": "cookie", "qty": 1},
                {"item": "spicy_sandwich", "qty": 1}, {"item": "polynesian", "qty": 2}]}),
        );
        assert_eq!(r.outcome, Outcome::Unconfirmed, "{r:#?}");
        assert_eq!(r.reason.as_deref(), Some("heard only part of the order"));
    }

    #[test]
    fn implied_count_needs_a_clear_transcript() {
        let ticket = serde_json::json!({"lines": [{"item": "polynesian", "qty": 2}]});
        assert_eq!(
            check_at(&["polynesian sauce please"], 0.7, ticket.clone()).outcome,
            Outcome::Unconfirmed
        );
        assert_eq!(
            check_at(&["polynesian sauce please"], 0.95, ticket).outcome,
            Outcome::Mismatch
        );
    }

    #[test]
    fn low_confidence_goes_unconfirmed() {
        let r = check_at(
            SPICY,
            0.4,
            serde_json::json!({"lines": [meal("chicken_sandwich", "sweet_tea"), {"item": "polynesian", "qty": 1}]}),
        );
        assert_eq!(r.outcome, Outcome::Unconfirmed);
        assert!(r.mismatches.is_empty());
        assert_eq!(r.unconfirmed.len(), 2);
    }

    #[test]
    fn silence_is_unconfirmed() {
        let r = check(
            &[],
            serde_json::json!({"lines": [meal("spicy_sandwich", "sweet_tea"), {"item": "polynesian", "qty": 3}, {"item": "cookie", "qty": 1}]}),
        );
        assert_eq!(r.outcome, Outcome::Unconfirmed);
        assert_eq!(r.reason.as_deref(), Some("nothing heard"));
        assert!(r.mismatches.is_empty());
    }

    #[test]
    fn ticket_validate_rejects_unknown() {
        let menu = cfa();
        let bad: Ticket =
            serde_json::from_value(serde_json::json!({"lines": [{"item": "big_mac", "qty": 1}]}))
                .unwrap();
        assert!(format!("{:#}", bad.validate(&menu).unwrap_err()).contains("big_mac"));
        let bad_mod: Ticket = serde_json::from_value(
            serde_json::json!({"lines": [{"item": "cookie", "qty": 1, "modifiers": ["no_ice"]}]}),
        )
        .unwrap();
        assert!(bad_mod.validate(&menu).is_err());
        let zero: Ticket =
            serde_json::from_value(serde_json::json!({"lines": [{"item": "cookie", "qty": 0}]}))
                .unwrap();
        assert!(zero.validate(&menu).is_err());
    }
}
