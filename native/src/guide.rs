// The in-app guide (ui/guide.json), so setup help is there without GitHub.
use crate::GuideItem;
use slint::{ModelRc, SharedString, VecModel};

#[derive(serde::Deserialize)]
struct Item {
    icon: String,
    title: String,
    intro: String,
    steps: Vec<String>,
}

pub fn items(lang: &str) -> Vec<GuideItem> {
    let all: std::collections::HashMap<String, Vec<Item>> = serde_json::from_str(include_str!("../ui/guide.json")).expect("guide.json");
    all.get(lang)
        .or_else(|| all.get("en"))
        .map(|v| {
            v.iter()
                .map(|i| GuideItem {
                    icon: i.icon.clone().into(),
                    title: i.title.clone().into(),
                    intro: i.intro.clone().into(),
                    steps: ModelRc::new(VecModel::from(i.steps.iter().map(|s| SharedString::from(s.as_str())).collect::<Vec<_>>())),
                })
                .collect()
        })
        .unwrap_or_default()
}
