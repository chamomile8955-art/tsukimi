use strsim::jaro_winkler;

use crate::ui::{models::SETTINGS, provider::descriptor::DescriptorType};

pub fn make_video_version_choice_from_filter(dl_list: Vec<String>) -> Option<usize> {
    let descriptors = crate::ui::models::SETTINGS.preferred_version_descriptors();
    let mut current_list: Vec<_> = dl_list.iter().collect();

    for descriptor in descriptors {
        let content = &descriptor.content.to_lowercase();
        let previous_list = current_list.to_owned();

        current_list.retain(|&name| match descriptor.type_ {
            DescriptorType::String => name.to_lowercase().contains(content),
            DescriptorType::Regex => {
                regex::Regex::new(content).is_ok_and(|re| re.is_match(&name.to_lowercase()))
            }
        });

        if current_list.is_empty() {
            current_list = previous_list; // Revert to the previous list
        }
    }

    current_list
        .first()
        .and_then(|first_item| dl_list.iter().position(|name| name == *first_item))
}

pub fn make_video_version_choice_from_matcher(
    dl_list: Vec<String>, matcher: &str,
) -> Option<usize> {
    let mut best_match_index = None;
    let mut highest_similarity = 0.0;
    for (index, name) in dl_list.iter().enumerate() {
        let similarity = jaro_winkler(name, matcher);
        if similarity > highest_similarity {
            highest_similarity = similarity;
            best_match_index = Some(index);
        }
    }

    best_match_index
}

pub fn make_subtitle_version_choice(lang_list: Vec<(i64, String)>) -> Option<(i64, usize)> {
    let mut preferences = vec![SubtitlePreference::ChineseSimplified];
    if let Some(configured) = SubtitlePreference::from_settings(SETTINGS.mpv_subtitle_preferred_lang())
        && configured != SubtitlePreference::ChineseSimplified
    {
        preferences.push(configured);
    }
    preferences.push(SubtitlePreference::ChineseTraditional);

    for preference in preferences {
        if let Some(choice) = best_subtitle_match(&lang_list, preference) {
            return Some(choice);
        }
    }

    None
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum SubtitlePreference {
    English,
    ChineseSimplified,
    Japanese,
    ChineseTraditional,
    Arabic,
    NorwegianBokmal,
    Portuguese,
    French,
    Russian,
}

impl SubtitlePreference {
    fn from_settings(value: i32) -> Option<Self> {
        match value {
            1 => Some(Self::English),
            2 => Some(Self::ChineseSimplified),
            3 => Some(Self::Japanese),
            4 => Some(Self::ChineseTraditional),
            5 => Some(Self::Arabic),
            6 => Some(Self::NorwegianBokmal),
            7 => Some(Self::Portuguese),
            8 => Some(Self::French),
            9 => Some(Self::Russian),
            _ => None,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::English => "English",
            Self::ChineseSimplified => "Chinese Simplified",
            Self::Japanese => "Japanese",
            Self::ChineseTraditional => "Chinese Traditional",
            Self::Arabic => "Arabic",
            Self::NorwegianBokmal => "Norwegian Bokmal",
            Self::Portuguese => "Portuguese",
            Self::French => "French",
            Self::Russian => "Russian",
        }
    }
}

fn best_subtitle_match(
    lang_list: &[(i64, String)], preference: SubtitlePreference,
) -> Option<(i64, usize)> {
    let mut best_match_index = None;
    let mut best_match_usize = None;
    let mut highest_similarity = 0.0;
    for (index, i) in lang_list.iter().enumerate() {
        let similarity = subtitle_match_score(&i.1, preference);
        if similarity > highest_similarity {
            highest_similarity = similarity;
            best_match_index = Some(i.0);
            best_match_usize = Some(index);
        }
    }

    (highest_similarity > 0.0).then_some((best_match_index?, best_match_usize?))
}

fn subtitle_match_score(text: &str, preference: SubtitlePreference) -> f64 {
    let normalized = text.to_lowercase().replace(['_', '-'], " ");
    match preference {
        SubtitlePreference::ChineseSimplified => {
            if contains_any(
                &normalized,
                &[
                    "chinese simplified",
                    "simplified chinese",
                    "simplified",
                    "简体",
                    "简中",
                    "chs",
                    "zh hans",
                    "zh cn",
                    "zh sg",
                ],
            ) {
                1.0
            } else if contains_any(&normalized, &["chinese", "中文", "汉语", "chi", "zho", "zh"])
                && !contains_any(
                    &normalized,
                    &[
                        "traditional",
                        "繁体",
                        "繁體",
                        "cht",
                        "zh hant",
                        "zh tw",
                        "zh hk",
                    ],
                )
            {
                0.82
            } else {
                0.0
            }
        }
        SubtitlePreference::ChineseTraditional => {
            if contains_any(
                &normalized,
                &[
                    "chinese traditional",
                    "traditional chinese",
                    "traditional",
                    "繁体",
                    "繁體",
                    "cht",
                    "zh hant",
                    "zh tw",
                    "zh hk",
                ],
            ) {
                1.0
            } else {
                0.0
            }
        }
        preference => jaro_winkler(&normalized, &preference.label().to_lowercase()),
    }
}

fn contains_any(text: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| text.contains(needle))
}
