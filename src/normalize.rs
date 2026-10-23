//! Turns raw transcript text into the token form menu aliases are written in.

const NUMBER_WORDS: [&str; 21] = [
    "zero", "one", "two", "three", "four", "five", "six", "seven", "eight", "nine", "ten", "eleven", "twelve",
    "thirteen", "fourteen", "fifteen", "sixteen", "seventeen", "eighteen", "nineteen", "twenty",
];

/// Words that mean "count" when they follow a number ("12 piece", "8ct").
const COUNT_WORDS: [&str; 6] = ["count", "ct", "piece", "pieces", "pc", "pcs"];

/// Lowercased tokens with brand spelling, numbers and piece counts unified,
/// so "a twelve-piece" and "a 12 ct" both become `a 12 count`.
pub fn normalize(text: &str) -> Vec<String> {
    let text = text
        .to_lowercase()
        .replace("chick-fil-a", "chick fil a")
        .replace("chickfila", "chick fil a")
        .replace("chick-fil a", "chick fil a")
        .replace('&', " and ")
        .replace(['-', '/'], " ");

    let mut raw = Vec::new();
    for word in text.split_whitespace() {
        let word: String = word.chars().filter(|c| c.is_alphanumeric() || *c == '\'').collect();
        let word = word.trim_matches('\'');
        if word.is_empty() {
            continue;
        }
        // "8ct" -> "8" "ct"
        let digits: String = word.chars().take_while(|c| c.is_ascii_digit()).collect();
        if !digits.is_empty() && digits.len() < word.len() {
            raw.push(digits.clone());
            raw.push(word[digits.len()..].to_string());
        } else {
            raw.push(word.to_string());
        }
    }

    let mut out: Vec<String> = Vec::with_capacity(raw.len());
    let mut i = 0;
    while i < raw.len() {
        let w = raw[i].as_str();
        if w == "a" && raw.get(i + 1).is_some_and(|n| n == "couple") {
            out.push("2".into());
            i += if raw.get(i + 2).is_some_and(|n| n == "of") { 3 } else { 2 };
            continue;
        }
        if let Some(n) = NUMBER_WORDS.iter().position(|nw| *nw == w) {
            out.push(n.to_string());
        } else if COUNT_WORDS.contains(&w) && out.last().is_some_and(|p| p.chars().all(|c| c.is_ascii_digit())) {
            out.push("count".into());
        } else {
            out.push(w.to_string());
        }
        i += 1;
    }
    out
}

/// Splits text after `.`, `?` and `!`, keeping the punctuation.
pub fn split_sentences(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    for c in text.chars() {
        cur.push(c);
        if matches!(c, '.' | '?' | '!') && !ends_with_abbreviation(&cur) {
            push_trimmed(&mut out, &cur);
            cur.clear();
        }
    }
    push_trimmed(&mut out, &cur);
    out
}

fn ends_with_abbreviation(s: &str) -> bool {
    let last = s.trim_end_matches('.').rsplit(|c: char| !c.is_alphanumeric()).next().unwrap_or("");
    s.ends_with('.') && ["dr", "mr", "mrs", "ms", "st"].contains(&last.to_lowercase().as_str())
}

fn push_trimmed(out: &mut Vec<String>, s: &str) {
    let t = s.trim();
    if !t.is_empty() && t.chars().any(|c| c.is_alphanumeric()) {
        out.push(t.to_string());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn n(s: &str) -> Vec<String> {
        normalize(s)
    }

    #[test]
    fn numbers_and_punctuation() {
        assert_eq!(n("Can I get THREE Polynesian sauces?"), ["can", "i", "get", "3", "polynesian", "sauces"]);
    }

    #[test]
    fn piece_counts() {
        assert_eq!(n("a twelve-piece nugget"), ["a", "12", "count", "nugget"]);
        assert_eq!(n("an 8ct and a 12-count"), ["an", "8", "count", "and", "a", "12", "count"]);
        assert_eq!(n("8 pc nuggets"), ["8", "count", "nuggets"]);
    }

    #[test]
    fn brand_and_ampersand() {
        assert_eq!(n("Chick-fil-A sauce & a large lemonade."), ["chick", "fil", "a", "sauce", "and", "a", "large", "lemonade"]);
        assert_eq!(n("chickfila sauce"), ["chick", "fil", "a", "sauce"]);
    }

    #[test]
    fn a_couple() {
        assert_eq!(n("a couple cookies"), ["2", "cookies"]);
        assert_eq!(n("a couple of cookies"), ["2", "cookies"]);
    }

    #[test]
    fn apostrophes_kept_inside_words() {
        assert_eq!(n("I'll take it, that's it"), ["i'll", "take", "it", "that's", "it"]);
    }

    #[test]
    fn digits_with_punctuation() {
        assert_eq!(n("3, please"), ["3", "please"]);
    }

    #[test]
    fn sentences() {
        assert_eq!(split_sentences("Hi. Can I get a meal? Yes!"), ["Hi.", "Can I get a meal?", "Yes!"]);
        assert_eq!(split_sentences("no punctuation here"), ["no punctuation here"]);
        assert!(split_sentences("   ").is_empty());
    }

    #[test]
    fn abbreviation_is_not_a_sentence_end() {
        assert_eq!(split_sentences("A large Dr. Pepper please. Thanks"), ["A large Dr. Pepper please.", "Thanks"]);
    }
}
