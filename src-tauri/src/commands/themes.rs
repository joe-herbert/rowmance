/// Tauri commands for managing user-created theme files.
/// Themes are stored as JSON files at ~/.config/rowmance/themes/<name>.json.
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tauri::command;

use crate::error::AppError;

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct ThemeMeta {
    pub name: String,
    pub extends: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ThemeData {
    pub name: String,
    pub extends: String,
    pub variables: HashMap<String, String>,
}

fn themes_dir() -> Result<PathBuf, AppError> {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .map(PathBuf::from)
        .map_err(|_| AppError::new("IO_ERROR", "Could not determine home directory"))?;
    Ok(home.join(".config").join("rowmance").join("themes"))
}

fn theme_path(dir: &Path, name: &str) -> PathBuf {
    dir.join(format!("{name}.json"))
}

// ── Internal directory-parameterised helpers (public for tests) ───────────────

pub(crate) fn list_in_dir(dir: &Path) -> Result<Vec<ThemeMeta>, AppError> {
    if !dir.exists() {
        return Ok(vec![]);
    }
    let mut metas = Vec::new();
    for entry in std::fs::read_dir(dir).map_err(|e| AppError::new("IO_ERROR", e.to_string()))? {
        let entry = entry.map_err(|e| AppError::new("IO_ERROR", e.to_string()))?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let raw = std::fs::read_to_string(&path)
            .map_err(|e| AppError::new("IO_ERROR", e.to_string()))?;
        if let Ok(data) = serde_json::from_str::<ThemeData>(&raw) {
            metas.push(ThemeMeta { name: data.name, extends: data.extends });
        }
    }
    metas.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(metas)
}

pub(crate) fn read_in_dir(dir: &Path, name: &str) -> Result<ThemeData, AppError> {
    let path = theme_path(dir, name);
    let raw = std::fs::read_to_string(&path)
        .map_err(|e| AppError::new("IO_ERROR", e.to_string()))?;
    serde_json::from_str(&raw).map_err(|e| AppError::new("SERIALISATION_ERROR", e.to_string()))
}

pub(crate) fn write_in_dir(dir: &Path, name: &str, data: &ThemeData) -> Result<(), AppError> {
    std::fs::create_dir_all(dir).map_err(|e| AppError::new("IO_ERROR", e.to_string()))?;
    let path = theme_path(dir, name);
    let json = serde_json::to_string_pretty(data)
        .map_err(|e| AppError::new("SERIALISATION_ERROR", e.to_string()))?;
    std::fs::write(&path, json).map_err(|e| AppError::new("IO_ERROR", e.to_string()))
}

pub(crate) fn delete_in_dir(dir: &Path, name: &str) -> Result<(), AppError> {
    let path = theme_path(dir, name);
    if path.exists() {
        std::fs::remove_file(&path).map_err(|e| AppError::new("IO_ERROR", e.to_string()))?;
    }
    Ok(())
}

// ── Public Tauri commands ─────────────────────────────────────────────────────

#[command]
pub fn themes_list() -> Result<Vec<ThemeMeta>, AppError> {
    let dir = themes_dir()?;
    list_in_dir(&dir)
}

#[command]
pub fn themes_read(name: String) -> Result<ThemeData, AppError> {
    let dir = themes_dir()?;
    read_in_dir(&dir, &name)
}

#[command]
pub fn themes_write(name: String, data: ThemeData) -> Result<(), AppError> {
    let dir = themes_dir()?;
    write_in_dir(&dir, &name, &data)
}

#[command]
pub fn themes_delete(name: String) -> Result<(), AppError> {
    let dir = themes_dir()?;
    delete_in_dir(&dir, &name)
}

#[command]
pub fn themes_rename(old_name: String, new_name: String) -> Result<ThemeMeta, AppError> {
    let dir = themes_dir()?;
    let mut data = read_in_dir(&dir, &old_name)?;
    data.name = new_name.clone();
    write_in_dir(&dir, &new_name, &data)?;
    if old_name != new_name {
        delete_in_dir(&dir, &old_name)?;
    }
    Ok(ThemeMeta { name: new_name, extends: data.extends })
}

#[command]
pub fn themes_duplicate(source: String, new_name: String) -> Result<ThemeMeta, AppError> {
    let dir = themes_dir()?;
    // If source doesn't exist on disk it's a built-in theme; start with empty variables.
    let mut data = read_in_dir(&dir, &source).unwrap_or_else(|_| ThemeData {
        name: source.clone(),
        extends: source.clone(),
        variables: HashMap::new(),
    });
    data.name = new_name.clone();
    write_in_dir(&dir, &new_name, &data)?;
    Ok(ThemeMeta { name: new_name, extends: data.extends })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn make_theme(name: &str, extends: &str) -> ThemeData {
        ThemeData { name: name.to_owned(), extends: extends.to_owned(), variables: HashMap::new() }
    }

    fn seed(dir: &Path, name: &str, extends: &str) {
        write_in_dir(dir, name, &make_theme(name, extends)).unwrap();
    }

    #[test]
    fn list_returns_empty_when_dir_absent() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("themes");
        assert_eq!(list_in_dir(&dir).unwrap(), vec![]);
    }

    #[test]
    fn write_then_read_roundtrip() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("themes");
        let mut vars = HashMap::new();
        vars.insert("--color-accent".to_owned(), "#ff0000".to_owned());
        let data = ThemeData { name: "my-theme".to_owned(), extends: "dark".to_owned(), variables: vars };
        write_in_dir(&dir, "my-theme", &data).unwrap();
        let read_back = read_in_dir(&dir, "my-theme").unwrap();
        assert_eq!(read_back.name, "my-theme");
        assert_eq!(read_back.extends, "dark");
        assert_eq!(read_back.variables.get("--color-accent").unwrap(), "#ff0000");
    }

    #[test]
    fn list_returns_all_themes_sorted() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("themes");
        seed(&dir, "zebra", "light");
        seed(&dir, "aardvark", "dark");
        let metas = list_in_dir(&dir).unwrap();
        assert_eq!(metas.len(), 2);
        assert_eq!(metas[0].name, "aardvark");
        assert_eq!(metas[1].name, "zebra");
    }

    #[test]
    fn list_ignores_non_json_files() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("themes");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("readme.txt"), "ignore me").unwrap();
        seed(&dir, "valid", "dark");
        let metas = list_in_dir(&dir).unwrap();
        assert_eq!(metas.len(), 1);
        assert_eq!(metas[0].name, "valid");
    }

    #[test]
    fn delete_removes_theme_file() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("themes");
        seed(&dir, "to-delete", "dark");
        assert_eq!(list_in_dir(&dir).unwrap().len(), 1);
        delete_in_dir(&dir, "to-delete").unwrap();
        assert_eq!(list_in_dir(&dir).unwrap().len(), 0);
    }

    #[test]
    fn delete_nonexistent_theme_is_ok() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("themes");
        assert!(delete_in_dir(&dir, "ghost").is_ok());
    }

    #[test]
    fn duplicate_creates_new_theme_with_new_name() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("themes");
        seed(&dir, "source", "light");
        let mut data = read_in_dir(&dir, "source").unwrap();
        data.name = "copy".to_owned();
        write_in_dir(&dir, "copy", &data).unwrap();
        let meta = ThemeMeta { name: "copy".to_owned(), extends: data.extends.clone() };
        assert_eq!(meta.name, "copy");
        assert_eq!(meta.extends, "light");
        let metas = list_in_dir(&dir).unwrap();
        assert_eq!(metas.len(), 2);
        let names: Vec<_> = metas.iter().map(|m| m.name.as_str()).collect();
        assert!(names.contains(&"source"));
        assert!(names.contains(&"copy"));
    }
}
