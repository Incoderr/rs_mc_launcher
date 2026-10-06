//! Quick Play uses the save folder identifier, not the world's display name.
use serde_json::Value;

/// A UI hint for official version IDs. Launch-time metadata remains authoritative.
pub fn potentially_supported(version: &str) -> bool {
    if let Some((year, week)) = version.split_once('w') {
        return year
            .parse::<u32>()
            .ok()
            .zip(week.get(..2).and_then(|week| week.parse::<u32>().ok()))
            .is_some_and(|(year, week)| (year, week) >= (23, 14));
    }
    let mut parts = version.split(['.', '-']);
    match (
        parts.next().and_then(|p| p.parse::<u32>().ok()),
        parts.next().and_then(|p| p.parse::<u32>().ok()),
    ) {
        (Some(1), Some(minor)) => minor >= 20,
        (Some(major), _) => major >= 26,
        _ => false,
    }
}

pub fn supports_singleplayer(metadata: &Value) -> bool {
    fn contains_flag(value: &Value) -> bool {
        match value {
            Value::String(value) => value == "--quickPlaySingleplayer",
            Value::Array(values) => values.iter().any(contains_flag),
            Value::Object(fields) => fields.get("value").is_some_and(contains_flag),
            _ => false,
        }
    }
    metadata
        .pointer("/arguments/game")
        .is_some_and(contains_flag)
}

pub fn singleplayer_arguments(metadata: &Value, folder: &str) -> Result<Vec<String>, &'static str> {
    if !supports_singleplayer(metadata) {
        return Err("Эта версия Minecraft не поддерживает Quick Play");
    }
    if folder.is_empty()
        || folder == "."
        || folder == ".."
        || folder.contains(['/', '\\', ':', '\0'])
    {
        return Err("Некорректное имя папки мира");
    }
    Ok(vec!["--quickPlaySingleplayer".into(), folder.into()])
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn conditional_metadata_and_world_names_are_preserved() {
        for version in [
            "1.20",
            "1.21.1",
            "1.20-pre1",
            "26.1",
            "26.1-snapshot-1",
            "23w14a",
        ] {
            assert!(potentially_supported(version));
        }
        for version in ["1.7.10", "1.19.4", "23w13a", "unknown"] {
            assert!(!potentially_supported(version));
        }
        let metadata = serde_json::json!({"arguments":{"game":[{"rules":[{"action":"allow","features":{"is_quick_play_singleplayer":true}}],"value":["--quickPlaySingleplayer","${quickPlaySingleplayer}"]}]}});
        assert_eq!(
            singleplayer_arguments(&metadata, "Мой мир 2").unwrap(),
            ["--quickPlaySingleplayer", "Мой мир 2"]
        );
        for name in ["", ".", "..", "../other", "C:\\world", "sub/world"] {
            assert!(singleplayer_arguments(&metadata, name).is_err());
        }
        assert!(
            singleplayer_arguments(
                &serde_json::json!({"minecraftArguments":"--username Player"}),
                "World"
            )
            .is_err()
        );
    }
}
