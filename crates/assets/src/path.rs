use formats::config::Language;
use rustc_hash::FxHashMap;
use std::collections::hash_map::Entry;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
pub fn fold(stored: &[u8]) -> String {
    let mut out = String::with_capacity(stored.len());
    let mut prev_sep = true;
    for &b in stored {
        if b == b'\\' || b == b'/' {
            if !prev_sep {
                out.push('/');
            }
            prev_sep = true;
        } else {
            out.push(b.to_ascii_lowercase() as char);
            prev_sep = false;
        }
    }
    if out.ends_with('/') {
        out.pop();
    }
    out.trim_start_matches("./").to_string()
}
#[derive(Debug, Clone)]
pub struct GameDir {
    root: PathBuf,
    index: Arc<FxHashMap<String, String>>,
    collisions: Vec<(String, String)>,
    language: Language,
    reads: Arc<AtomicUsize>,
}
impl GameDir {
    pub fn open(root: impl Into<PathBuf>) -> std::io::Result<GameDir> {
        let root = root.into();
        if !root.is_dir() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("game directory {} does not exist", root.display()),
            ));
        }
        let mut index = FxHashMap::default();
        let mut collisions = Vec::new();
        walk(&root, &mut String::new(), &mut index, &mut collisions);
        Ok(GameDir {
            root,
            index: Arc::new(index),
            collisions,
            language: Language::English,
            reads: Arc::new(AtomicUsize::new(0)),
        })
    }
    pub fn root(&self) -> &Path {
        &self.root
    }
    pub fn language(&self) -> Language {
        self.language
    }
    pub fn set_language(&mut self, language: Language) {
        self.language = language;
    }
    pub fn len(&self) -> usize {
        self.index.len()
    }
    pub fn is_empty(&self) -> bool {
        self.index.is_empty()
    }
    pub fn case_collisions(&self) -> &[(String, String)] {
        &self.collisions
    }
    pub fn resolve(&self, stored: &[u8]) -> Option<PathBuf> {
        let actual = self.index.get(&fold(stored))?;
        Some(self.root.join(actual))
    }
    pub fn contains(&self, stored: &[u8]) -> bool {
        self.index.contains_key(&fold(stored))
    }
    pub fn reads(&self) -> usize {
        self.reads.load(Ordering::Relaxed)
    }
    pub fn read(&self, stored: &[u8]) -> std::io::Result<Vec<u8>> {
        self.reads.fetch_add(1, Ordering::Relaxed);
        match self.resolve(stored) {
            Some(p) => std::fs::read(p),
            None => Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!(
                    "{} is not in {}",
                    String::from_utf8_lossy(stored),
                    self.root.display()
                ),
            )),
        }
    }
    pub fn resolve_lang(&self, name: &str) -> Option<PathBuf> {
        self.resolve_lang_in(self.language, name)
    }
    pub fn resolve_lang_in(&self, language: Language, name: &str) -> Option<PathBuf> {
        let direct = format!("Lang\\{}\\{name}", language.dir());
        if let Some(p) = self.resolve(direct.as_bytes()) {
            return Some(p);
        }
        if language == Language::English {
            return None;
        }
        self.resolve(format!("Lang\\EN\\{name}").as_bytes())
    }
    pub fn read_lang(&self, name: &str) -> std::io::Result<Vec<u8>> {
        match self.resolve_lang(name) {
            Some(p) => std::fs::read(p),
            None => Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!(
                    "{name} is in neither Lang/{} nor Lang/EN",
                    self.language.dir()
                ),
            )),
        }
    }
    pub fn iter(&self) -> impl Iterator<Item = &str> {
        self.index.values().map(String::as_str)
    }
}
fn walk(
    dir: &Path,
    prefix: &mut String,
    index: &mut FxHashMap<String, String>,
    collisions: &mut Vec<(String, String)>,
) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let Ok(name) = entry.file_name().into_string() else {
            continue;
        };
        let restore = prefix.len();
        if !prefix.is_empty() {
            prefix.push('/');
        }
        prefix.push_str(&name);
        match entry.file_type() {
            Ok(t) if t.is_dir() => walk(&entry.path(), prefix, index, collisions),
            Ok(_) => {
                index_path(prefix, index, collisions);
            }
            Err(_) => {}
        }
        prefix.truncate(restore);
    }
}
fn index_path(
    path: &str,
    index: &mut FxHashMap<String, String>,
    collisions: &mut Vec<(String, String)>,
) {
    match index.entry(fold(path.as_bytes())) {
        Entry::Vacant(v) => {
            v.insert(path.to_owned());
        }
        Entry::Occupied(mut o) => {
            let (winner, loser) = if path < o.get().as_str() {
                (path.to_owned(), o.get().clone())
            } else {
                (o.get().clone(), path.to_owned())
            };
            let key = o.key().clone();
            o.insert(winner);
            collisions.push((key, loser));
        }
    }
}
