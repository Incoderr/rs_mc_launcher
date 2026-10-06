use std::{io, path::Path};

pub fn list(saves: &Path) -> io::Result<Vec<String>> {
    let entries = match std::fs::read_dir(saves) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error),
    };
    let mut worlds = Vec::new();
    for entry in entries {
        let entry = entry?;
        if entry.file_type()?.is_dir()
            && entry.path().join("level.dat").is_file()
            && let Ok(name) = entry.file_name().into_string()
        {
            worlds.push(name);
        }
    }
    worlds.sort_by_key(|name| name.to_lowercase());
    Ok(worlds)
}

pub fn validate(saves: &Path, folder: &str) -> io::Result<()> {
    let root = saves.canonicalize()?;
    let world = saves.join(folder).canonicalize()?;
    if world.parent() != Some(root.as_path()) || !world.join("level.dat").is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Мир отсутствует в папке сохранений сборки",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_world_folders_are_listed_and_outside_paths_are_rejected() {
        let directory = tempfile::tempdir().unwrap();
        let saves = directory.path().join("saves");
        assert!(list(&saves).unwrap().is_empty());
        std::fs::create_dir_all(saves.join("Мой мир")).unwrap();
        std::fs::create_dir_all(saves.join("unfinished")).unwrap();
        std::fs::write(saves.join("Мой мир/level.dat"), b"world").unwrap();
        std::fs::create_dir_all(directory.path().join("other")).unwrap();
        std::fs::write(directory.path().join("other/level.dat"), b"world").unwrap();
        assert_eq!(list(&saves).unwrap(), ["Мой мир"]);
        assert!(validate(&saves, "Мой мир").is_ok());
        assert!(validate(&saves, "../other").is_err());
        assert!(validate(&saves, "missing").is_err());
    }
}
