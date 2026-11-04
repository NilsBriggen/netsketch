//! The interface language. English text is its own key: `tr("Save")` gives "Speichern" in German.
//! Translations live in data/de.json; anything missing there stays English.
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::OnceLock;

/// 0 = English, 1 = German.
static LANG: AtomicUsize = AtomicUsize::new(0);

pub const LANGUAGES: [&str; 2] = ["English", "Deutsch"];

pub fn set(lang: usize) {
    LANG.store(lang.min(LANGUAGES.len() - 1), Ordering::Relaxed);
}

pub fn german() -> bool {
    LANG.load(Ordering::Relaxed) == 1
}

fn de() -> &'static HashMap<String, String> {
    static DE: OnceLock<HashMap<String, String>> = OnceLock::new();
    DE.get_or_init(|| serde_json::from_str(include_str!("../data/de.json")).expect("data/de.json is checked by the tests"))
}

/// This text in the chosen language.
pub fn tr(en: &str) -> &str {
    if german() {
        if let Some(t) = de().get(en) {
            return t;
        }
    }
    en
}

/// A translated template with `{}` filled in from left to right: `trf("{} devices", &[&n])`.
pub fn trf(en: &str, args: &[&dyn std::fmt::Display]) -> String {
    let mut out = String::new();
    let mut parts = tr(en).split("{}");
    out += parts.next().unwrap_or("");
    for (i, p) in parts.enumerate() {
        if let Some(a) = args.get(i) {
            out += &a.to_string();
        }
        out += p;
    }
    out
}

#[cfg(test)]
mod tests {
    #[test]
    fn german_keeps_every_placeholder_and_falls_back_to_english() {
        for (en, de) in super::de() {
            assert!(!de.trim().is_empty(), "'{en}' has an empty translation");
            assert_eq!(en.matches("{}").count(), de.matches("{}").count(), "'{en}' → '{de}' loses a placeholder");
        }
        assert_eq!(super::tr("no such text"), "no such text");
    }
}
