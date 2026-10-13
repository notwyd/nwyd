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

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::ErrorKind;

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            static NEXT: AtomicUsize = AtomicUsize::new(0);
            loop {
                let path = std::env::temp_dir().join(format!(
                    "nwyd-assets-test-{}-{}",
                    std::process::id(),
                    NEXT.fetch_add(1, Ordering::Relaxed)
                ));
                match std::fs::create_dir(&path) {
                    Ok(()) => return Self(path),
                    Err(error) if error.kind() == ErrorKind::AlreadyExists => {}
                    Err(error) => panic!("creating test directory: {error}"),
                }
            }
        }

        fn write(&self, name: &str, bytes: &[u8]) {
            let path = self.0.join(name);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, bytes).unwrap();
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn lookup_keys_normalize_case_separators_and_dot_prefixes() {
        for path in [
            b"UI/Panel.BMP".as_slice(),
            b"\\UI\\\\Panel.BMP\\",
            b"././ui//panel.bmp/",
            b".\\.\\UI\\Panel.bmp",
        ] {
            assert_eq!(fold(path), "ui/panel.bmp");
        }
        assert_eq!(fold(b""), "");
        assert_eq!(fold(b"/\\//"), "");
        assert_eq!(fold(b"././"), ".");
    }

    #[test]
    fn lookup_keys_preserve_non_ascii_bytes_and_parent_components() {
        assert_eq!(fold(b"UI/\xc9.BMP"), "ui/É.bmp");
        assert_eq!(fold(b"UI/../Panel.BMP"), "ui/../panel.bmp");
    }

    #[test]
    fn indexes_nested_files_and_preserves_actual_spelling() {
        let root = TestDirectory::new();
        root.write("UI/Panel.BMP", b"synthetic image");
        root.write("Lang/EN/Labels.txt", b"synthetic text");
        let game = GameDir::open(&root.0).unwrap();
        assert_eq!(game.len(), 2);
        assert!(!game.is_empty());
        assert!(!game.contains(b"UI"));
        assert!(game.contains(b".\\ui\\panel.bmp"));
        assert_eq!(
            game.resolve(b"UI/PANEL.BMP"),
            Some(root.0.join("UI/Panel.BMP"))
        );
        let mut files: Vec<_> = game.iter().collect();
        files.sort_unstable();
        assert_eq!(files, ["Lang/EN/Labels.txt", "UI/Panel.BMP"]);
    }

    #[test]
    fn empty_directory_has_no_indexed_assets() {
        let root = TestDirectory::new();
        let game = GameDir::open(&root.0).unwrap();
        assert!(game.is_empty());
        assert_eq!(game.len(), 0);
        assert!(game.resolve(b"absent.bin").is_none());
    }

    #[test]
    fn open_rejects_missing_roots_and_regular_files() {
        let root = TestDirectory::new();
        root.write("file.bin", b"synthetic");
        for path in [root.0.join("missing"), root.0.join("file.bin")] {
            assert_eq!(GameDir::open(path).unwrap_err().kind(), ErrorKind::NotFound);
        }
    }

    #[test]
    fn reads_binary_assets_and_counts_failed_attempts_across_clones() {
        let root = TestDirectory::new();
        root.write("Data.bin", b"\0NWYD\xff");
        let game = GameDir::open(&root.0).unwrap();
        let clone = game.clone();
        assert_eq!(game.read(b"data.BIN").unwrap(), b"\0NWYD\xff");
        assert_eq!(
            clone.read(b"missing.bin").unwrap_err().kind(),
            ErrorKind::NotFound
        );
        assert_eq!(game.reads(), 2);
        assert_eq!(clone.reads(), 2);
    }

    #[test]
    fn indexed_file_removed_before_read_returns_an_io_error() {
        let root = TestDirectory::new();
        root.write("Data.bin", b"synthetic");
        let game = GameDir::open(&root.0).unwrap();
        std::fs::remove_file(root.0.join("Data.bin")).unwrap();
        assert_eq!(
            game.read(b"data.bin").unwrap_err().kind(),
            ErrorKind::NotFound
        );
    }

    #[test]
    fn selected_language_takes_precedence_over_english() {
        let root = TestDirectory::new();
        root.write("Lang/EN/Labels.txt", b"english");
        root.write("Lang/PT/Labels.txt", b"portuguese");
        let mut game = GameDir::open(&root.0).unwrap();
        let english = game.clone();
        game.set_language(Language::Portuguese);
        assert_eq!(game.read_lang("labels.TXT").unwrap(), b"portuguese");
        assert_eq!(english.read_lang("labels.txt").unwrap(), b"english");
    }

    #[test]
    fn missing_translation_falls_back_to_english() {
        let root = TestDirectory::new();
        root.write("Lang/EN/Labels.txt", b"english");
        let mut game = GameDir::open(&root.0).unwrap();
        game.set_language(Language::Portuguese);
        assert_eq!(game.read_lang("Labels.txt").unwrap(), b"english");
        assert_eq!(
            game.resolve_lang_in(Language::Turkish, "labels.txt"),
            Some(root.0.join("Lang/EN/Labels.txt"))
        );
    }

    #[test]
    fn language_lookup_does_not_fall_back_to_an_unrelated_translation() {
        let root = TestDirectory::new();
        root.write("Lang/PT/Labels.txt", b"portuguese");
        let mut game = GameDir::open(&root.0).unwrap();
        assert_eq!(
            game.read_lang("Labels.txt").unwrap_err().kind(),
            ErrorKind::NotFound
        );
        game.set_language(Language::Turkish);
        assert_eq!(
            game.read_lang("Labels.txt").unwrap_err().kind(),
            ErrorKind::NotFound
        );
    }

    #[test]
    fn case_collision_winner_does_not_depend_on_directory_order() {
        for paths in [["ui/A.bmp", "UI/a.bmp"], ["UI/a.bmp", "ui/A.bmp"]] {
            let mut index = FxHashMap::default();
            let mut collisions = Vec::new();
            for path in paths {
                index_path(path, &mut index, &mut collisions);
            }
            assert_eq!(index.len(), 1);
            assert_eq!(index["ui/a.bmp"], "UI/a.bmp");
            assert_eq!(collisions, [("ui/a.bmp".to_owned(), "ui/A.bmp".to_owned())]);
        }
    }
}
