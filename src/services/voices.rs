use crate::models::api::VoiceDescriptor;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet, HashMap};

fn normalize_voice_key(voice: &str) -> String {
    voice.to_lowercase()
}

fn voice_lookup<'a, I>(available_voices: I) -> HashMap<String, String>
where
    I: IntoIterator<Item = &'a str>,
{
    available_voices
        .into_iter()
        .map(|voice| (normalize_voice_key(voice), voice.to_string()))
        .collect()
}

fn match_available_voice(
    voice_id: &str,
    available_voices: &HashMap<String, String>,
) -> Option<String> {
    available_voices
        .get(&normalize_voice_key(voice_id))
        .cloned()
}

type ResolveVoiceFn = fn(Option<&str>, &BTreeMap<String, String>, &[String], &str) -> String;
type BuildVoiceDescriptorsFn = fn(&[String], &BTreeMap<String, String>) -> Vec<VoiceDescriptor>;

pub(crate) fn resolve_voice(
    requested_voice_id: Option<&str>,
    voice_map: &BTreeMap<String, String>,
    available_voices: &[String],
    default_voice_id: &str,
) -> String {
    let available = voice_lookup(available_voices.iter().map(String::as_str));
    let default_voice = match_available_voice(default_voice_id, &available)
        .unwrap_or_else(|| default_voice_id.to_string());

    match requested_voice_id {
        Some(requested_voice_id) if !requested_voice_id.is_empty() => {
            // Preserve the Python lookup order exactly: alias map first,
            // case-insensitive direct match second, default fallback last.
            if let Some(mapped_voice) = voice_map.get(requested_voice_id) {
                return match_available_voice(mapped_voice, &available)
                    .unwrap_or_else(|| mapped_voice.clone());
            }

            match_available_voice(requested_voice_id, &available).unwrap_or(default_voice)
        }
        _ => default_voice,
    }
}

const _: ResolveVoiceFn = resolve_voice;

pub(crate) fn build_voice_descriptors(
    available_voices: &[String],
    voice_map: &BTreeMap<String, String>,
) -> Vec<VoiceDescriptor> {
    let mut aliases_by_voice: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (alias, kitten_voice) in voice_map {
        aliases_by_voice
            .entry(normalize_voice_key(kitten_voice))
            .or_default()
            .push(alias.clone());
    }

    for aliases in aliases_by_voice.values_mut() {
        aliases.sort();
    }

    let unique_voices: BTreeSet<String> = available_voices.iter().cloned().collect();
    let mut sorted_voices: Vec<String> = unique_voices.into_iter().collect();
    sorted_voices.sort_by(|left, right| {
        normalize_voice_key(left)
            .cmp(&normalize_voice_key(right))
            .then_with(|| left.cmp(right))
    });

    sorted_voices
        .into_iter()
        .map(|voice| {
            let canonical_voice_id = normalize_voice_key(&voice);
            let aliases = aliases_by_voice
                .get(&canonical_voice_id)
                .cloned()
                .unwrap_or_default();

            let mut labels = BTreeMap::new();
            labels.insert(
                "provider".to_string(),
                Value::String("KittenTTS".to_string()),
            );
            labels.insert("source".to_string(), Value::String("local".to_string()));
            labels.insert("kitten_voice".to_string(), Value::String(voice.clone()));
            if !aliases.is_empty() {
                labels.insert(
                    "aliases".to_string(),
                    Value::Array(aliases.iter().cloned().map(Value::String).collect()),
                );
            }

            let description = if aliases.is_empty() {
                format!("Local KittenTTS voice {voice}.")
            } else {
                format!(
                    "Local KittenTTS voice {voice}. Also reachable via aliases: {}.",
                    aliases.join(", ")
                )
            };

            VoiceDescriptor {
                voice_id: canonical_voice_id,
                name: voice,
                category: "premade".to_string(),
                description: Some(description),
                preview_url: None,
                available_for_tiers: vec!["local".to_string()],
                labels,
            }
        })
        .collect()
}

const _: BuildVoiceDescriptorsFn = build_voice_descriptors;

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn voice_map(entries: &[(&str, &str)]) -> BTreeMap<String, String> {
        entries
            .iter()
            .map(|(alias, voice)| (alias.to_string(), voice.to_string()))
            .collect()
    }

    fn available_voices(entries: &[&str]) -> Vec<String> {
        entries.iter().map(|voice| voice.to_string()).collect()
    }

    #[test]
    fn resolve_voice_prefers_alias_mapping() {
        let resolved = resolve_voice(
            Some("Jasper"),
            &voice_map(&[("Jasper", "Bella")]),
            &available_voices(&["Jasper", "Bella"]),
            "jasper",
        );

        assert_eq!(resolved, "Bella");
    }

    #[test]
    fn resolve_voice_matches_available_voice_case_insensitively() {
        let resolved = resolve_voice(
            Some("jAsPeR"),
            &BTreeMap::new(),
            &available_voices(&["Jasper", "Bella"]),
            "bella",
        );

        assert_eq!(resolved, "Jasper");
    }

    #[test]
    fn resolve_voice_uses_default_when_request_is_missing() {
        let resolved = resolve_voice(
            None,
            &BTreeMap::new(),
            &available_voices(&["Jasper", "Bella"]),
            "jAsPeR",
        );

        assert_eq!(resolved, "Jasper");
    }

    #[test]
    fn resolve_voice_falls_back_to_default_for_unknown_voice() {
        let resolved = resolve_voice(
            Some("unknown"),
            &BTreeMap::new(),
            &available_voices(&["Jasper", "Bella"]),
            "bella",
        );

        assert_eq!(resolved, "Bella");
    }

    #[test]
    fn build_voice_descriptors_includes_alias_metadata() {
        let descriptors = build_voice_descriptors(
            &available_voices(&["Bella", "Jasper"]),
            &voice_map(&[("friendly", "Bella"), ("fallback", "Bella")]),
        );

        assert_eq!(descriptors.len(), 2);
        assert_eq!(descriptors[0].voice_id, "bella");
        assert_eq!(descriptors[0].name, "Bella");
        assert_eq!(descriptors[0].category, "premade");
        assert_eq!(
            descriptors[0].available_for_tiers,
            vec!["local".to_string()]
        );
        assert_eq!(
            descriptors[0].labels.get("provider"),
            Some(&json!("KittenTTS"))
        );
        assert_eq!(descriptors[0].labels.get("source"), Some(&json!("local")));
        assert_eq!(
            descriptors[0].labels.get("kitten_voice"),
            Some(&json!("Bella"))
        );
        assert_eq!(
            descriptors[0].labels.get("aliases"),
            Some(&json!(["fallback", "friendly"]))
        );
        assert_eq!(
            descriptors[0].description.as_deref(),
            Some("Local KittenTTS voice Bella. Also reachable via aliases: fallback, friendly.")
        );
    }
}
