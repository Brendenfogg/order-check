//! Optional service-standards check: one true/false per standard per order.

use std::collections::BTreeMap;

use crate::extract::{SpokenOrder, Utterance};
use crate::menu::{Menu, StandardKind};
use crate::normalize::normalize;

pub fn check_standards(
    menu: &Menu,
    utts: &[Utterance],
    spoken: &SpokenOrder,
) -> BTreeMap<String, bool> {
    let heard: Vec<Vec<String>> = utts.iter().map(|u| normalize(&u.text)).collect();
    menu.standards
        .iter()
        .map(|std| {
            let met = match std.kind {
                StandardKind::Phrase => std.phrases.iter().any(|p| {
                    let p = normalize(p);
                    !p.is_empty()
                        && heard
                            .iter()
                            .any(|toks| toks.windows(p.len()).any(|w| w == p.as_slice()))
                }),
                StandardKind::OfferLto => {
                    spoken.offered.iter().any(|id| menu.lto_items.contains(id))
                }
            };
            (std.id.clone(), met)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::extract::{extract, utts};
    use crate::matcher::Matcher;
    use crate::menu::cfa;

    fn run(lines: &[&str]) -> BTreeMap<String, bool> {
        let menu = cfa();
        let u = utts(lines, 0.95);
        let spoken = extract(&menu, &Matcher::new(&menu), &u);
        check_standards(&menu, &u, &spoken)
    }

    #[test]
    fn standards_met() {
        let r = run(&[
            "Can I get a spicy chicken sandwich meal?",
            "Would you like to try our seasonal peach milkshake today?",
            "No thanks.",
            "My pleasure!",
        ]);
        assert_eq!(
            r,
            BTreeMap::from([("my_pleasure".into(), true), ("offered_lto".into(), true)])
        );
    }

    #[test]
    fn standards_missed() {
        let r = run(&["Hi, a lemonade please", "Thanks, we'll bring it out"]);
        assert_eq!(
            r,
            BTreeMap::from([("my_pleasure".into(), false), ("offered_lto".into(), false)])
        );
    }

    #[test]
    fn guest_naming_the_lto_is_not_an_offer() {
        let r = run(&["Can I get a peach milkshake?"]);
        assert_eq!(r["offered_lto"], false);
    }
}
