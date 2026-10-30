// UI strings (ui/strings.json, shared with the guide texts).
use std::collections::HashMap;

pub struct I18n {
    pub lang: String,
    table: HashMap<String, String>,
    fallback: HashMap<String, String>,
}

impl I18n {
    pub fn new(pref: &str) -> Self {
        let all: HashMap<String, HashMap<String, String>> =
            serde_json::from_str(include_str!("../ui/strings.json")).expect("strings.json");
        let lang = resolve(pref);
        Self {
            table: all.get(&lang).cloned().unwrap_or_default(),
            fallback: all.get("en").cloned().unwrap_or_default(),
            lang,
        }
    }

    pub fn t(&self, key: &str) -> String {
        self.table
            .get(key)
            .or_else(|| self.fallback.get(key))
            .cloned()
            .unwrap_or_else(|| key.to_string())
    }

    pub fn tv(&self, key: &str, vars: &[(&str, &str)]) -> String {
        let mut s = self.t(key);
        for (k, v) in vars {
            s = s.replace(&format!("{{{k}}}"), v);
        }
        s
    }
}

fn resolve(pref: &str) -> String {
    if pref == "en" || pref == "ru" {
        return pref.into();
    }
    let loc = crate::platform::locale().to_lowercase();
    if ["ru", "uk", "be", "kk"].iter().any(|p| loc.starts_with(p)) {
        "ru".into()
    } else {
        "en".into()
    }
}
