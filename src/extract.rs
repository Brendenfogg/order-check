//! Turns what was said during an order into a structured spoken order.

use serde::{Deserialize, Serialize};

use crate::matcher::{Cue, Matcher, Mention, MentionKind};
use crate::menu::{Category, Menu, Size};
use crate::normalize::{normalize, split_sentences};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Utterance {
    pub text: String,
    pub confidence: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SpokenComponent {
    pub item: Option<String>,
    pub family: String,
    pub size: Option<Size>,
    pub modifiers: Vec<String>,
    pub confidence: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SpokenMeal {
    pub side: Option<SpokenComponent>,
    pub drink: Option<SpokenComponent>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SpokenLine {
    pub item: Option<String>,
    pub family: String,
    pub qty: Option<u32>,
    /// The quantity was actually said (a number or "a"), not just implied.
    #[serde(skip)]
    pub qty_said: bool,
    /// Which sentence first named this line; two mentions in one sentence are two items.
    #[serde(skip)]
    pub sentence: usize,
    pub size: Option<Size>,
    pub modifiers: Vec<String>,
    pub meal: Option<SpokenMeal>,
    pub meal_size: Option<Size>,
    pub confidence: f32,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize)]
pub struct SpokenOrder {
    pub lines: Vec<SpokenLine>,
    pub offered: Vec<String>,
}

/// Words that put a side into a meal instead of ordering it separately.
const SIDE_SWAP_WORDS: &[&str] = &["with", "sub", "substitute"];
/// "sub mac and cheese for the fries": the item after these is being replaced, not ordered.
const SWAP_SOURCE: &[&[&str]] = &[
    &["for", "the"],
    &["for"],
    &["instead", "of"],
    &["instead", "of", "the"],
];

pub fn extract(menu: &Menu, matcher: &Matcher, utts: &[Utterance]) -> SpokenOrder {
    let mut ex = Extractor {
        menu,
        order: SpokenOrder::default(),
        pending_offer: Vec::new(),
        sentence_no: 0,
        open_meal: None,
        meal_age: 0,
        target: None,
    };
    for u in utts {
        for sentence in split_sentences(&u.text) {
            let tokens = normalize(&sentence);
            let mentions = matcher.find(&tokens);
            ex.sentence(&tokens, &mentions, u.confidence);
        }
    }
    ex.order
}

/// Words that make "actually, can I also get…" an addition, not a correction.
const ADD_WORDS: &[&str] = &["also", "add", "too", "plus"];
/// A reply that starts with one of these turns an offer down, whatever follows.
const LEADING_NO: &[&str] = &["no", "nah", "nope"];

/// Something an employee offered, held until the guest answers.
#[derive(Debug, Clone)]
enum Offered {
    Item(Phrase),
    Meal,
    Size(Size),
    Qty(u32),
}

/// "Sure." accepts. "No thanks", "Yeah, no, I'm good" and "No, I'll take a
/// lemonade" don't, and neither does "I'll take <something not offered>".
fn accepts_offer(tokens: &[String], units: &[Unit], pending: &[Offered]) -> bool {
    if tokens
        .first()
        .is_some_and(|t| LEADING_NO.contains(&t.as_str()))
    {
        return false;
    }
    if units
        .iter()
        .any(|u| matches!(u, Unit::Other(MentionKind::Cue(Cue::Decline))))
    {
        return false;
    }
    if !matches!(
        units.first(),
        Some(Unit::Other(MentionKind::Cue(Cue::Accept)))
    ) {
        return false;
    }
    match units.get(1) {
        Some(Unit::Phrase(p)) => pending.iter().any(|o| match (o, &p.kind) {
            (Offered::Item(op), PhraseKind::Item(id)) => {
                matches!(&op.kind, PhraseKind::Item(oid) if oid == id)
            }
            _ => false,
        }),
        _ => true,
    }
}

/// What a correction or modifier applies to: the last thing mentioned.
#[derive(Debug, Clone, Copy)]
enum Target {
    Line(usize),
    Drink(usize),
    Side(usize),
}

#[derive(Debug, Clone)]
enum PhraseKind {
    Item(String),
    Family(String),
    Base,
}

/// An item mention together with the quantity, size and meal words around it.
#[derive(Debug, Clone)]
struct Phrase {
    kind: PhraseKind,
    qty: Option<u32>,
    qty_from_number: bool,
    qty_said: bool,
    size: Option<Size>,
    meal: bool,
    meal_size: Option<Size>,
    side_swap: bool,
    score: f32,
}

#[derive(Debug)]
enum Unit {
    Phrase(Phrase),
    Other(MentionKind),
}

fn adjacent(a: &Mention, b: &Mention) -> bool {
    a.end == b.start
}

fn is_head(k: &MentionKind) -> bool {
    matches!(
        k,
        MentionKind::Item(_) | MentionKind::Family(_) | MentionKind::FamilyBase
    )
}

/// Groups each item mention with its quantity/size/meal words, in mention order.
fn group(menu: &Menu, tokens: &[String], ms: &[Mention]) -> Vec<Unit> {
    let n = ms.len();
    let mut used = vec![false; n];
    let mut phrases: Vec<Option<Phrase>> = vec![None; n];
    for j in 0..n {
        if !is_head(&ms[j].kind) || used[j] {
            continue;
        }
        // "regular fries": "regular" is just an adjective on the next item.
        if ms[j].kind == MentionKind::FamilyBase
            && j + 1 < n
            && adjacent(&ms[j], &ms[j + 1])
            && is_head(&ms[j + 1].kind)
        {
            used[j] = true;
            continue;
        }
        let mut p = Phrase {
            kind: match &ms[j].kind {
                MentionKind::Item(id) => PhraseKind::Item(id.clone()),
                MentionKind::Family(id) => PhraseKind::Family(id.clone()),
                _ => PhraseKind::Base,
            },
            qty: None,
            qty_from_number: false,
            qty_said: false,
            size: None,
            meal: false,
            meal_size: None,
            side_swap: false,
            score: ms[j].score,
        };
        let mut first = j;
        if first > 0
            && !used[first - 1]
            && adjacent(&ms[first - 1], &ms[first])
            && let MentionKind::Size(s) = ms[first - 1].kind
        {
            p.size = Some(s);
            used[first - 1] = true;
            first -= 1;
        }
        if first > 0 && !used[first - 1] && adjacent(&ms[first - 1], &ms[first]) {
            match ms[first - 1].kind {
                MentionKind::Quantity(q) => {
                    p.qty = Some(q);
                    p.qty_from_number = true;
                    p.qty_said = true;
                    used[first - 1] = true;
                    first -= 1;
                }
                MentionKind::Article => {
                    p.qty = Some(1);
                    p.qty_said = true;
                    used[first - 1] = true;
                    first -= 1;
                }
                _ => {}
            }
        }
        // A singular item named with no number means one ("spicy deluxe meal"),
        // but "some polynesian sauce" leaves the amount open.
        let after_some = ms[first].start > 0 && tokens[ms[first].start - 1] == "some";
        if p.qty.is_none() && !ms[j].plural && !after_some && matches!(p.kind, PhraseKind::Item(_))
        {
            p.qty = Some(1);
        }
        let start_tok = ms[first].start;
        let is_swap_source = SWAP_SOURCE.iter().any(|w| {
            start_tok >= w.len()
                && tokens[start_tok - w.len()..start_tok]
                    .iter()
                    .zip(w.iter())
                    .all(|(a, b)| a == b)
        });
        if is_swap_source && phrase_category(menu, &p) == Some(Category::Side) {
            for u in used.iter_mut().take(j + 1).skip(first) {
                *u = true;
            }
            continue;
        }
        p.side_swap = (1..=2).any(|back| {
            start_tok >= back && SIDE_SWAP_WORDS.contains(&tokens[start_tok - back].as_str())
        });

        let mut next = j + 1;
        if next < n && ms[next].kind == MentionKind::Meal && ms[next].start - ms[j].end <= 1 {
            p.meal = true;
            used[next] = true;
            next += 1;
            if is_entree(menu, &p) {
                p.meal_size = p.size.take();
            }
        }
        // "a lemonade, large" — unless the size belongs to the next item.
        if next < n
            && !used[next]
            && adjacent(&ms[next - 1], &ms[next])
            && let MentionKind::Size(s) = ms[next].kind
        {
            let starts_next_item =
                next + 1 < n && adjacent(&ms[next], &ms[next + 1]) && is_head(&ms[next + 1].kind);
            if !starts_next_item {
                if p.meal {
                    p.meal_size = p.meal_size.or(Some(s))
                } else {
                    p.size = p.size.or(Some(s))
                }
                used[next] = true;
            }
        }
        used[j] = true;
        phrases[j] = Some(p);
    }

    let mut units = Vec::new();
    for j in 0..n {
        if let Some(p) = phrases[j].take() {
            units.push(Unit::Phrase(p));
        } else if !used[j] {
            units.push(Unit::Other(ms[j].kind.clone()));
        }
    }
    units
}

fn phrase_category(menu: &Menu, p: &Phrase) -> Option<Category> {
    match &p.kind {
        PhraseKind::Item(id) => menu.item(id).map(|i| i.category),
        PhraseKind::Family(f) => menu
            .family(f)
            .and_then(|f| menu.item(&f.base))
            .map(|i| i.category),
        PhraseKind::Base => None,
    }
}

fn is_entree(menu: &Menu, p: &Phrase) -> bool {
    phrase_category(menu, p) == Some(Category::Entree)
}

struct Extractor<'a> {
    menu: &'a Menu,
    order: SpokenOrder,
    pending_offer: Vec<Offered>,
    sentence_no: usize,
    open_meal: Option<usize>,
    meal_age: u8,
    target: Option<Target>,
}

impl<'a> Extractor<'a> {
    fn sentence(&mut self, tokens: &[String], ms: &[Mention], conf: f32) {
        self.sentence_no += 1;
        // A meal's drink can come in the same sentence or the next one.
        if self.open_meal.is_some() {
            self.meal_age += 1;
            if self.meal_age > 1 {
                self.open_meal = None;
            }
        }
        let units = group(self.menu, tokens, ms);

        // An offer ("would you like to make that a meal?") only changes the
        // order if the very next reply accepts it.
        let pending = std::mem::take(&mut self.pending_offer);
        if !pending.is_empty() && accepts_offer(tokens, &units, &pending) {
            for edit in pending {
                match edit {
                    Offered::Item(p) => self.add(p, conf, false),
                    Offered::Meal => self.make_meal(),
                    Offered::Size(s) => self.correct_size(s),
                    Offered::Qty(q) => self.set_qty(q),
                }
            }
        }

        // "Actually, can I also get…" adds; it doesn't correct.
        let adds = tokens.iter().any(|t| ADD_WORDS.contains(&t.as_str()));
        let (mut offer, mut correcting, mut removing, mut additive, mut negate, mut every) =
            (false, false, false, false, false, false);
        let mut touched: Vec<Target> = Vec::new();
        for unit in units {
            match unit {
                Unit::Other(MentionKind::Cue(cue)) => match cue {
                    Cue::Offer => offer = true,
                    Cue::Correction => correcting = !adds,
                    Cue::Removal => removing = true,
                    Cue::Additive => additive = true,
                    Cue::Negation => negate = true,
                    Cue::Every => every = true,
                    Cue::Accept | Cue::Decline => {}
                },
                Unit::Other(MentionKind::Modifier(id)) => self.attach_modifier(&id),
                Unit::Other(MentionKind::Meal) if offer => self.pending_offer.push(Offered::Meal),
                Unit::Other(MentionKind::Size(s)) if offer => {
                    self.pending_offer.push(Offered::Size(s))
                }
                Unit::Other(MentionKind::Quantity(q)) if offer => {
                    self.pending_offer.push(Offered::Qty(q))
                }
                Unit::Other(MentionKind::Meal) if correcting => self.make_meal(),
                Unit::Other(MentionKind::Size(s)) if correcting => self.correct_size(s),
                Unit::Other(MentionKind::Size(s)) if every => {
                    for t in &touched {
                        self.fill_size(*t, s);
                    }
                    every = false;
                }
                Unit::Other(MentionKind::Quantity(q)) if correcting => self.set_qty(q),
                Unit::Other(_) => {}
                Unit::Phrase(mut p) => {
                    if negate {
                        negate = false;
                        continue;
                    }
                    if let PhraseKind::Base = p.kind {
                        match (correcting, self.target_family()) {
                            (true, Some(f)) => match self.menu.family(&f) {
                                Some(fam) => p.kind = PhraseKind::Item(fam.base.clone()),
                                None => continue,
                            },
                            _ => continue,
                        }
                    }
                    if offer {
                        if let PhraseKind::Item(id) = &p.kind {
                            self.order.offered.push(id.clone());
                        }
                        self.pending_offer.push(Offered::Item(p));
                    } else if removing {
                        removing = false;
                        // "Never mind, just a lemonade": nothing to remove by
                        // that name, so drop the last item and take this one.
                        if !self.remove(&p) {
                            self.remove_last();
                            self.add(p, conf, false);
                            touched.extend(self.target);
                        }
                    } else if correcting && self.correct_with(&p, conf) {
                        correcting = false;
                    } else {
                        if additive {
                            // "one more cookie" / "another cookie" is one more, even with no number.
                            if p.qty.is_none() || !p.qty_said {
                                p.qty = Some(1);
                                p.qty_said = true;
                            }
                        }
                        self.add(p, conf, additive);
                        additive = false;
                        touched.extend(self.target);
                    }
                }
            }
        }
    }

    fn set_qty(&mut self, q: u32) {
        if let Some(i) = self.target_line() {
            self.order.lines[i].qty = Some(q);
            self.order.lines[i].qty_said = true;
        }
    }

    fn family_of(&self, p: &Phrase) -> String {
        match &p.kind {
            PhraseKind::Item(id) => self
                .menu
                .item(id)
                .map(|i| i.family.clone())
                .unwrap_or_default(),
            PhraseKind::Family(f) => f.clone(),
            PhraseKind::Base => String::new(),
        }
    }

    fn item_of(p: &Phrase) -> Option<String> {
        match &p.kind {
            PhraseKind::Item(id) => Some(id.clone()),
            _ => None,
        }
    }

    fn component(&self, p: &Phrase, conf: f32) -> SpokenComponent {
        SpokenComponent {
            item: Self::item_of(p),
            family: self.family_of(p),
            size: p.size,
            modifiers: Vec::new(),
            confidence: conf.min(p.score),
        }
    }

    fn compatible(&self, item: &Option<String>, family: &str, p: &Phrase) -> bool {
        match (item, Self::item_of(p)) {
            (Some(a), Some(b)) => *a == b,
            _ => family == self.family_of(p),
        }
    }

    fn add(&mut self, p: Phrase, conf: f32, additive: bool) {
        let conf = conf.min(p.score);
        let category = phrase_category(self.menu, &p);

        if !additive {
            if let Some(mi) = self.open_meal
                && self.attach_to_meal(mi, &p, category, conf)
            {
                return;
            }
            if let Some(i) = self.find_merge(&p) {
                self.merge(i, &p, conf);
                return;
            }
            // A read-back of a meal drink after the meal window closed.
            if category == Some(Category::Drink) {
                for i in (0..self.order.lines.len()).rev() {
                    let matches = self.order.lines[i]
                        .meal
                        .as_ref()
                        .and_then(|m| m.drink.as_ref())
                        .is_some_and(|d| self.compatible(&d.item, &d.family, &p));
                    if matches {
                        self.merge_component(i, true, &p, conf);
                        return;
                    }
                }
            }
        }

        let line = SpokenLine {
            item: Self::item_of(&p),
            family: self.family_of(&p),
            qty: p.qty,
            qty_said: p.qty_said,
            sentence: self.sentence_no,
            size: p.size,
            modifiers: Vec::new(),
            meal: p.meal.then_some(SpokenMeal {
                side: None,
                drink: None,
            }),
            meal_size: p.meal_size,
            confidence: conf,
        };
        self.order.lines.push(line);
        let i = self.order.lines.len() - 1;
        self.target = Some(Target::Line(i));
        if category == Some(Category::Entree) {
            self.open_meal = p.meal.then_some(i);
            self.meal_age = 0;
        }
    }

    /// Puts a drink (or a "with"-side) into the open meal. Returns true if it did.
    fn attach_to_meal(
        &mut self,
        mi: usize,
        p: &Phrase,
        category: Option<Category>,
        conf: f32,
    ) -> bool {
        let is_drink = category == Some(Category::Drink);
        let is_side = category == Some(Category::Side);
        if !is_drink && !is_side {
            return false;
        }
        let Some(meal) = self.order.lines[mi].meal.as_ref() else {
            return false;
        };
        let slot = if is_drink { &meal.drink } else { &meal.side };
        match slot {
            Some(existing) if self.compatible(&existing.item, &existing.family, p) => {
                self.merge_component(mi, is_drink, p, conf);
                true
            }
            Some(_) => false,
            None if is_drink || p.side_swap => {
                let mut comp = self.component(p, conf);
                let mut mi = mi;
                // A read-back that names the drink with the meal means the
                // drink said earlier on its own was the meal's drink.
                let standalone = (0..self.order.lines.len()).find(|&j| {
                    let l = &self.order.lines[j];
                    j != mi
                        && l.meal.is_none()
                        && l.item
                            .as_deref()
                            .and_then(|id| self.menu.item(id))
                            .map(|i| i.category)
                            == category
                        && self.compatible(&l.item, &l.family, p)
                });
                if let Some(j) = standalone {
                    let l = self.order.lines.remove(j);
                    if j < mi {
                        mi -= 1;
                    }
                    comp.item = comp.item.or(l.item);
                    comp.size = l.size.or(comp.size);
                    comp.modifiers = l.modifiers;
                    comp.confidence = comp.confidence.max(l.confidence);
                    self.open_meal = Some(mi);
                }
                let meal = self.order.lines[mi].meal.as_mut().unwrap();
                if is_drink {
                    meal.drink = Some(comp)
                } else {
                    meal.side = Some(comp)
                }
                self.target = Some(if is_drink {
                    Target::Drink(mi)
                } else {
                    Target::Side(mi)
                });
                true
            }
            None => false,
        }
    }

    fn merge_component(&mut self, i: usize, drink: bool, p: &Phrase, conf: f32) {
        let item = Self::item_of(p);
        let meal = self.order.lines[i].meal.as_mut().unwrap();
        let c = if drink {
            meal.drink.as_mut()
        } else {
            meal.side.as_mut()
        };
        if let Some(c) = c {
            c.item = c.item.take().or(item);
            c.size = c.size.or(p.size);
            c.confidence = c.confidence.max(conf.min(p.score));
        }
        self.target = Some(if drink {
            Target::Drink(i)
        } else {
            Target::Side(i)
        });
    }

    fn find_merge(&self, p: &Phrase) -> Option<usize> {
        (0..self.order.lines.len()).rev().find(|&i| {
            let l = &self.order.lines[i];
            if !self.compatible(&l.item, &l.family, p) {
                return false;
            }
            let same_item = l.item.is_some() && l.item == Self::item_of(p);
            let conflict = matches!((l.qty, p.qty), (Some(a), Some(b)) if a != b)
                || matches!((l.size, p.size), (Some(a), Some(b)) if a != b);
            // A conflicting repeat is a read-back only if it came in a later sentence.
            if conflict && l.sentence == self.sentence_no {
                return false;
            }
            same_item || !conflict
        })
    }

    /// Fills in what the earlier mention left unsaid; the first statement wins.
    fn merge(&mut self, i: usize, p: &Phrase, conf: f32) {
        let item = Self::item_of(p);
        let l = &mut self.order.lines[i];
        l.item = l.item.take().or(item);
        if !l.qty_said && p.qty_said {
            // A spoken number beats one that was only implied.
            l.qty = p.qty;
            l.qty_said = true;
        } else {
            l.qty = l.qty.or(p.qty);
        }
        l.size = l.size.or(p.size);
        l.meal_size = l.meal_size.or(p.meal_size);
        if p.meal && l.meal.is_none() {
            l.meal = Some(SpokenMeal {
                side: None,
                drink: None,
            });
        }
        l.confidence = l.confidence.max(conf.min(p.score));
        self.target = Some(Target::Line(i));
        if l.meal.is_some() {
            self.open_meal = Some(i);
            self.meal_age = 0;
        }
    }

    fn target_line(&self) -> Option<usize> {
        match self.target? {
            Target::Line(i) | Target::Drink(i) | Target::Side(i) => Some(i),
        }
    }

    fn target_family(&self) -> Option<String> {
        let t = self.target?;
        let l = self.order.lines.get(self.target_line()?)?;
        Some(match t {
            Target::Line(_) => l.family.clone(),
            Target::Drink(_) => l.meal.as_ref()?.drink.as_ref()?.family.clone(),
            Target::Side(_) => l.meal.as_ref()?.side.as_ref()?.family.clone(),
        })
    }

    /// Applies a correction that names an item. Returns false if nothing fits.
    fn correct_with(&mut self, p: &Phrase, conf: f32) -> bool {
        let family = self.family_of(p);
        let item = Self::item_of(p);
        let conf = conf.min(p.score);
        // A meal component in the same family, most recent first.
        for i in (0..self.order.lines.len()).rev() {
            let l = &mut self.order.lines[i];
            if l.family == family {
                if item.is_some() {
                    l.item = item.clone();
                }
                if p.qty_from_number {
                    l.qty = p.qty;
                }
                if p.size.is_some() {
                    l.size = p.size;
                }
                if p.meal && l.meal.is_none() {
                    l.meal = Some(SpokenMeal {
                        side: None,
                        drink: None,
                    });
                }
                l.confidence = conf;
                self.target = Some(Target::Line(i));
                return true;
            }
            if let Some(meal) = l.meal.as_mut() {
                for (is_drink, slot) in [(true, &mut meal.drink), (false, &mut meal.side)] {
                    if let Some(c) = slot.as_mut().filter(|c| c.family == family) {
                        if item.is_some() {
                            c.item = item.clone();
                        }
                        if p.size.is_some() {
                            c.size = p.size;
                        }
                        self.target = Some(if is_drink {
                            Target::Drink(i)
                        } else {
                            Target::Side(i)
                        });
                        return true;
                    }
                }
            }
        }
        // Different family but same kind of thing ("make that a lemonade" after a tea).
        let category = phrase_category(self.menu, p);
        if let Some(Target::Line(i)) = self.target {
            let l = &self.order.lines[i];
            let l_cat = l
                .item
                .as_deref()
                .and_then(|id| self.menu.item(id))
                .map(|it| it.category);
            if l_cat.is_some() && l_cat == category {
                let l = &mut self.order.lines[i];
                l.item = item;
                l.family = family;
                if p.size.is_some() {
                    l.size = p.size;
                }
                return true;
            }
        }
        false
    }

    /// Sets a size on a sized item that doesn't have one yet ("both medium").
    fn fill_size(&mut self, t: Target, s: Size) {
        let menu = self.menu;
        let sized = |id: &Option<String>| {
            id.as_deref()
                .and_then(|id| menu.item(id))
                .is_some_and(|i| i.sized)
        };
        let Some(l) = self.order.lines.get_mut(match t {
            Target::Line(i) | Target::Drink(i) | Target::Side(i) => i,
        }) else {
            return;
        };
        match t {
            Target::Line(_) if sized(&l.item) && l.size.is_none() => l.size = Some(s),
            Target::Drink(_) | Target::Side(_) => {
                let meal = l.meal.as_mut();
                let c = meal.and_then(|m| {
                    if matches!(t, Target::Drink(_)) {
                        m.drink.as_mut()
                    } else {
                        m.side.as_mut()
                    }
                });
                if let Some(c) = c.filter(|c| c.size.is_none()) {
                    c.size = Some(s);
                }
            }
            _ => {}
        }
    }

    fn correct_size(&mut self, s: Size) {
        let Some(t) = self.target else { return };
        let menu = self.menu;
        let sized = |id: &Option<String>| {
            id.as_deref()
                .and_then(|id| menu.item(id))
                .is_some_and(|i| i.sized)
        };
        match t {
            Target::Line(i) => {
                let l = &mut self.order.lines[i];
                if l.meal.is_some() {
                    l.meal_size = Some(s);
                } else if sized(&l.item) || l.item.is_none() {
                    l.size = Some(s);
                }
            }
            Target::Drink(i) | Target::Side(i) => {
                let meal = self.order.lines[i].meal.as_mut().unwrap();
                let c = if matches!(t, Target::Drink(_)) {
                    meal.drink.as_mut()
                } else {
                    meal.side.as_mut()
                };
                if let Some(c) = c {
                    c.size = Some(s);
                }
            }
        }
    }

    fn make_meal(&mut self) {
        let Some(i) = self.target_line() else { return };
        let menu = self.menu;
        let l = &mut self.order.lines[i];
        let eligible = l
            .item
            .as_deref()
            .and_then(|id| menu.item(id))
            .map(|it| it.meal)
            .unwrap_or(true);
        if eligible && l.meal.is_none() {
            l.meal = Some(SpokenMeal {
                side: None,
                drink: None,
            });
            l.meal_size = l.meal_size.or(l.size.take());
        }
        self.open_meal = Some(i);
        self.meal_age = 0;
    }

    /// Removes the most recent line with this item. Returns false if there was none.
    fn remove(&mut self, p: &Phrase) -> bool {
        let pos = (0..self.order.lines.len()).rev().find(|&i| {
            let l = &self.order.lines[i];
            self.compatible(&l.item, &l.family, p)
        });
        let Some(i) = pos else { return false };
        self.order.lines.remove(i);
        self.target = None;
        self.open_meal = None;
        true
    }

    fn remove_last(&mut self) {
        if self.order.lines.pop().is_some() {
            self.target = None;
            self.open_meal = None;
        }
    }

    fn attach_modifier(&mut self, id: &str) {
        let menu = self.menu;
        let allows = |item: &Option<String>, family: &str| {
            let item_id = item
                .clone()
                .or_else(|| menu.family(family).map(|f| f.base.clone()));
            item_id
                .and_then(|i| menu.item(&i).map(|it| it.modifiers.iter().any(|m| m == id)))
                .unwrap_or(false)
        };
        let mut candidates: Vec<Target> = self.target.into_iter().collect();
        let n = self.order.lines.len();
        for i in (n.saturating_sub(2)..n).rev() {
            candidates.extend([Target::Drink(i), Target::Side(i), Target::Line(i)]);
        }
        for t in candidates {
            let Some(l) = self.order.lines.get_mut(match t {
                Target::Line(i) | Target::Drink(i) | Target::Side(i) => i,
            }) else {
                continue;
            };
            let mods = match t {
                Target::Line(_) => allows(&l.item, &l.family).then_some(&mut l.modifiers),
                Target::Drink(_) => l
                    .meal
                    .as_mut()
                    .and_then(|m| m.drink.as_mut())
                    .filter(|c| allows(&c.item, &c.family))
                    .map(|c| &mut c.modifiers),
                Target::Side(_) => l
                    .meal
                    .as_mut()
                    .and_then(|m| m.side.as_mut())
                    .filter(|c| allows(&c.item, &c.family))
                    .map(|c| &mut c.modifiers),
            };
            if let Some(mods) = mods {
                if !mods.iter().any(|m| m == id) {
                    mods.push(id.to_string());
                }
                return;
            }
        }
    }
}

#[cfg(test)]
pub(crate) fn utts(lines: &[&str], confidence: f32) -> Vec<Utterance> {
    lines
        .iter()
        .map(|t| Utterance {
            text: t.to_string(),
            confidence,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::menu::cfa;

    fn ex(lines: &[&str]) -> SpokenOrder {
        let menu = cfa();
        extract(&menu, &Matcher::new(&menu), &utts(lines, 0.95))
    }

    fn ids(o: &SpokenOrder) -> Vec<Option<&str>> {
        o.lines.iter().map(|l| l.item.as_deref()).collect()
    }

    fn drink(l: &SpokenLine) -> &SpokenComponent {
        l.meal
            .as_ref()
            .expect("meal")
            .drink
            .as_ref()
            .expect("meal drink")
    }

    #[test]
    fn spicy_meal_and_sauces() {
        let o = ex(&[
            "Welcome! What can I get started for you?",
            "Can I get a spicy chicken sandwich meal with a medium sweet tea?",
            "Would you like to try our seasonal peach milkshake today?",
            "No thanks. Oh, and can I get three Polynesian sauces?",
            "Three Polynesian, got it. Anything else?",
            "That's it.",
            "My pleasure!",
        ]);
        assert_eq!(ids(&o), [Some("spicy_sandwich"), Some("polynesian")]);
        assert_eq!(o.lines[0].qty, Some(1));
        let d = drink(&o.lines[0]);
        assert_eq!(
            (d.item.as_deref(), d.size),
            (Some("sweet_tea"), Some(Size::Md))
        );
        assert_eq!(o.lines[1].qty, Some(3));
        assert_eq!(o.offered, ["peach_shake"]);
    }

    #[test]
    fn accepted_offer() {
        let o = ex(&["Would you like to try our peach milkshake?", "Sure!"]);
        assert_eq!(ids(&o), [Some("peach_shake")]);
    }

    #[test]
    fn readback_conflict_first_wins() {
        let o = ex(&[
            "Can I get three Polynesian sauces",
            "One Polynesian, got it",
        ]);
        assert_eq!(ids(&o), [Some("polynesian")]);
        assert_eq!(o.lines[0].qty, Some(3));
    }

    #[test]
    fn headset_readback_only() {
        let o = ex(&[
            "So that's a 12 count nugget meal with a large lemonade and two chick fil a sauces?",
            "Pull forward please.",
        ]);
        assert_eq!(ids(&o), [Some("nuggets_12"), Some("cfa_sauce")]);
        let d = drink(&o.lines[0]);
        assert_eq!(
            (d.item.as_deref(), d.size),
            (Some("lemonade"), Some(Size::Lg))
        );
        assert_eq!(o.lines[1].qty, Some(2));
    }

    #[test]
    fn correction_size() {
        let o = ex(&["a medium lemonade", "actually make that a large"]);
        assert_eq!(ids(&o), [Some("lemonade")]);
        assert_eq!(o.lines[0].size, Some(Size::Lg));
    }

    #[test]
    fn correction_variant_regular() {
        let o = ex(&["a spicy chicken sandwich", "no wait, regular not spicy"]);
        assert_eq!(ids(&o), [Some("chicken_sandwich")]);
    }

    #[test]
    fn correction_variant_named() {
        let o = ex(&["a medium lemonade", "actually make that a diet lemonade"]);
        assert_eq!(ids(&o), [Some("diet_lemonade")]);
        assert_eq!(o.lines[0].size, Some(Size::Md));
    }

    #[test]
    fn make_it_a_meal() {
        let o = ex(&["8 count nuggets", "actually make it a meal"]);
        assert_eq!(ids(&o), [Some("nuggets_8")]);
        assert!(o.lines[0].meal.is_some());
    }

    #[test]
    fn removal() {
        let o = ex(&["a cookie and a lemonade", "never mind the cookie"]);
        assert_eq!(ids(&o), [Some("lemonade")]);
    }

    #[test]
    fn modifiers() {
        let o = ex(&["a chicken sandwich with no pickles and a large sweet tea no ice"]);
        assert_eq!(ids(&o), [Some("chicken_sandwich"), Some("sweet_tea")]);
        assert_eq!(o.lines[0].modifiers, ["no_pickles"]);
        assert_eq!(o.lines[1].size, Some(Size::Lg));
        assert_eq!(o.lines[1].modifiers, ["no_ice"]);
    }

    #[test]
    fn meal_drink_modifier() {
        let o = ex(&["a spicy deluxe meal with a large lemonade, light ice"]);
        assert_eq!(drink(&o.lines[0]).modifiers, ["light_ice"]);
    }

    #[test]
    fn additive_duplicate() {
        let o = ex(&["a cookie", "and another cookie"]);
        let total: u32 = o
            .lines
            .iter()
            .filter(|l| l.item.as_deref() == Some("cookie"))
            .map(|l| l.qty.unwrap_or(1))
            .sum();
        assert_eq!(total, 2);
    }

    #[test]
    fn unspecified() {
        let o = ex(&["some nuggets and a lemonade"]);
        assert_eq!(ids(&o), [None, Some("lemonade")]);
        assert_eq!(o.lines[0].family, "nuggets");
        assert_eq!(o.lines[0].qty, None);
        assert_eq!(o.lines[1].qty, Some(1));
        assert_eq!(o.lines[1].size, None);
    }

    #[test]
    fn meal_side_substitution() {
        let o = ex(&["a grilled chicken sandwich meal with mac and cheese and a sweet tea"]);
        let meal = o.lines[0].meal.as_ref().unwrap();
        assert_eq!(
            meal.side.as_ref().unwrap().item.as_deref(),
            Some("mac_and_cheese")
        );
        assert_eq!(
            meal.drink.as_ref().unwrap().item.as_deref(),
            Some("sweet_tea")
        );
        assert_eq!(o.lines.len(), 1);
    }

    #[test]
    fn large_meal_size() {
        let o = ex(&["a large 8 count nugget meal with a sunjoy"]);
        assert_eq!(o.lines[0].meal_size, Some(Size::Lg));
    }

    #[test]
    fn misheard_no_wait_still_corrects() {
        let o = ex(&["Let me get a spicy chicken sandwich, no weight, regular, not spicy"]);
        assert_eq!(ids(&o), [Some("chicken_sandwich")]);
    }

    #[test]
    fn substitution_source_is_not_ordered() {
        let o = ex(&[
            "A grilled chicken sandwich meal, but sub mac and cheese for the fries, and a medium unsweet tea",
        ]);
        assert_eq!(ids(&o), [Some("grilled_sandwich")]);
        let meal = o.lines[0].meal.as_ref().unwrap();
        assert_eq!(
            meal.side.as_ref().unwrap().item.as_deref(),
            Some("mac_and_cheese")
        );
    }

    #[test]
    fn one_more_then_readback_totals_two() {
        let o = ex(&[
            "a cookies and cream milkshake",
            "and one more cookies and cream",
            "so two cookies and cream",
        ]);
        let total: u32 = o
            .lines
            .iter()
            .filter(|l| l.item.as_deref() == Some("cookies_cream_shake"))
            .map(|l| l.qty.unwrap_or(1))
            .sum();
        assert_eq!(total, 2, "{o:#?}");
    }

    #[test]
    fn readback_moves_drink_into_meal() {
        let o = ex(&[
            "Chicken sandwich, add pepper jack, and a large coke",
            "Chicken sandwich meal, add pepper jack, large coke",
        ]);
        assert_eq!(ids(&o), [Some("chicken_sandwich")], "{o:#?}");
        let d = drink(&o.lines[0]);
        assert_eq!((d.item.as_deref(), d.size), (Some("cola"), Some(Size::Lg)));
    }

    #[test]
    fn chicken_strips_count_is_the_item_not_quantity() {
        let o = ex(&[
            "Hi, a 4-count chicken strips and 2 sriracha sauces",
            "4-count strips and 2 sriracha",
        ]);
        assert_eq!(ids(&o), [Some("strips_4"), Some("sriracha")]);
        assert_eq!(o.lines[0].qty, Some(1));
    }

    #[test]
    fn singular_item_without_number_is_one() {
        let o = ex(&["Large deluxe meal with a Dr Pepper, no tomato"]);
        assert_eq!(o.lines[0].qty, Some(1));
    }

    #[test]
    fn plural_without_number_stays_unspecified() {
        let o = ex(&["and polynesian sauces please"]);
        assert_eq!(o.lines[0].qty, None);
    }

    #[test]
    fn spoken_number_beats_implied_one() {
        let o = ex(&[
            "A strawberry shake & cookies & cream shake",
            "So 1 strawberry & 2 cookies & cream",
        ]);
        let cc = o
            .lines
            .iter()
            .find(|l| l.item.as_deref() == Some("cookies_cream_shake"))
            .unwrap();
        assert_eq!(cc.qty, Some(2));
    }

    #[test]
    fn both_size_applies_to_each_item() {
        let o = ex(&["One diet lemonade and one regular lemonade, both medium"]);
        assert!(o.lines.iter().all(|l| l.size == Some(Size::Md)), "{o:#?}");
    }

    #[test]
    fn grill_nuggets() {
        let o = ex(&["12-count grill nuggets and a large diet coke"]);
        assert_eq!(ids(&o), [Some("grilled_nuggets_12"), Some("diet_cola")]);
    }

    // ---- review findings: declined upsells must change nothing
    #[test]
    fn declined_meal_upsell_changes_nothing() {
        let o = ex(&[
            "A spicy chicken sandwich",
            "Would you like to make that a meal?",
            "No thanks.",
        ]);
        assert_eq!(ids(&o), [Some("spicy_sandwich")]);
        assert!(o.lines[0].meal.is_none(), "{o:#?}");
    }

    #[test]
    fn declined_combo_upsell_changes_nothing() {
        let o = ex(&[
            "A spicy chicken sandwich",
            "Do you want to make it a combo?",
            "No, just the sandwich.",
        ]);
        assert!(o.lines[0].meal.is_none(), "{o:#?}");
    }

    #[test]
    fn declined_size_upsell_changes_nothing() {
        let o = ex(&[
            "A medium lemonade",
            "Would you like to make that a large?",
            "No thanks.",
        ]);
        assert_eq!(o.lines[0].size, Some(Size::Md), "{o:#?}");
    }

    #[test]
    fn accepted_meal_upsell_applies() {
        let o = ex(&[
            "A spicy chicken sandwich",
            "Would you like to make that a meal?",
            "Sure.",
        ]);
        assert!(o.lines[0].meal.is_some(), "{o:#?}");
    }

    #[test]
    fn no_then_ordering_something_else_declines_offer() {
        let o = ex(&[
            "Would you like to try our peach milkshake?",
            "No, I'll take a large lemonade",
        ]);
        assert_eq!(ids(&o), [Some("lemonade")], "{o:#?}");
    }

    #[test]
    fn yeah_no_im_good_declines_offer() {
        let o = ex(&[
            "Would you like to try our peach milkshake?",
            "Yeah, no, I'm good",
        ]);
        assert!(o.lines.is_empty(), "{o:#?}");
    }

    // ---- review findings: "actually" and "never mind" reach
    #[test]
    fn actually_also_adds() {
        let o = ex(&[
            "A large sweet tea",
            "Actually, can I also get a medium lemonade?",
        ]);
        assert_eq!(ids(&o), [Some("sweet_tea"), Some("lemonade")], "{o:#?}");
    }

    #[test]
    fn actually_add_too_adds() {
        let o = ex(&[
            "A chicken sandwich",
            "Actually, can I add a spicy chicken sandwich too?",
        ]);
        assert_eq!(
            ids(&o),
            [Some("chicken_sandwich"), Some("spicy_sandwich")],
            "{o:#?}"
        );
    }

    #[test]
    fn never_mind_then_new_item_replaces() {
        let o = ex(&["A cookie.", "Never mind, just a lemonade."]);
        assert_eq!(ids(&o), [Some("lemonade")], "{o:#?}");
    }

    #[test]
    fn two_sizes_in_one_sentence_are_two_items() {
        let o = ex(&["a large lemonade and a small lemonade"]);
        assert_eq!(o.lines.len(), 2, "{o:#?}");
    }

    #[test]
    fn some_singular_is_unspecified() {
        let o = ex(&["a spicy chicken sandwich with some polynesian sauce"]);
        let poly = o
            .lines
            .iter()
            .find(|l| l.item.as_deref() == Some("polynesian"))
            .unwrap();
        assert_eq!(poly.qty, None);
    }

    #[test]
    fn confidence_is_min_of_utterance_and_match() {
        let menu = cfa();
        let o = extract(
            &menu,
            &Matcher::new(&menu),
            &utts(&["a large lemmonade"], 0.9),
        );
        assert!(o.lines[0].confidence < 0.9 && o.lines[0].confidence >= 0.85);
    }
}
