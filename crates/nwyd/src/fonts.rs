use assets::GameDir;
use formats::ui::guimat::FontDef;
use render::font::Face;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock, Weak};

pub const FALLBACK: &[u8] = include_bytes!("../assets/LiberationSans-Regular.ttf");

#[derive(Default)]
struct Faces {
    recent: Vec<(Box<[u8]>, Weak<Face>)>,
}

impl Faces {
    fn parse(&mut self, bytes: &[u8]) -> Result<Arc<Face>, String> {
        self.recent.retain(|(_, face)| face.strong_count() != 0);

        if let Some(index) = self
            .recent
            .iter()
            .position(|(source, _)| &**source == bytes)
        {
            let entry = self.recent.remove(index);

            if let Some(face) = entry.1.upgrade() {
                self.recent.push(entry);

                return Ok(face);
            }
        }

        let face = Arc::new(Face::parse(bytes, 0)?);

        if self.recent.len() == 8 {
            self.recent.remove(0);
        }

        self.recent.push((bytes.into(), Arc::downgrade(&face)));

        Ok(face)
    }
}

fn face(bytes: &[u8]) -> Result<Arc<Face>, String> {
    static FACES: OnceLock<Mutex<Faces>> = OnceLock::new();

    FACES
        .get_or_init(Default::default)
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .parse(bytes)
}

pub fn system_font_dir() -> Option<PathBuf> {
    if let Some(dir) = std::env::var_os("WYD_SYSTEM_FONTS") {
        let path = PathBuf::from(dir);

        return path.is_dir().then_some(path);
    }

    let windir = std::env::var_os("WINDIR").or_else(|| std::env::var_os("SystemRoot"))?;

    let path = Path::new(&windir).join("Fonts");

    path.is_dir().then_some(path)
}

#[derive(Debug, Default)]
pub struct Chain {
    pub faces: Vec<Arc<Face>>,
    pub sources: Vec<String>,
}

#[derive(Debug, Default)]
pub struct Chains {
    pub chains: [Chain; 2],
    pub notes: Vec<String>,
}

impl Chains {
    pub fn get(&self, face: i32) -> &Chain {
        &self.chains[usize::from(face == 1)]
    }
}

pub fn load(game: &GameDir, system: Option<&Path>) -> Chains {
    let mut out = Chains::default();

    let fallback = face(FALLBACK).ok();

    for (index, entries) in FontDef::FACE_CHAINS.iter().enumerate() {
        let chain = &mut out.chains[index];

        let mut primary_declared = None;

        for (slot, (family, file, from_system)) in entries.iter().enumerate() {
            let bytes = if *from_system {
                let name = file.rsplit(['/', '\\']).next().unwrap_or(file);

                system.and_then(|dir| std::fs::read(dir.join(name)).ok())
            } else {
                game.read(file.as_bytes()).ok()
            };

            let Some(bytes) = bytes else {
                if slot == 0 {
                    primary_declared = Some(*family);
                }

                continue;
            };

            match face(&bytes) {
                Ok(face) => {
                    chain.faces.push(face);

                    chain.sources.push((*file).to_string());
                }
                Err(reason) => {
                    if slot == 0 {
                        primary_declared = Some(*family);
                    }

                    tracing::warn!(
                        chain = index, font =* file, error =% reason,
                        "font face unavailable"
                    );

                    out.notes
                        .push(format!("chain {index}: {file} did not parse ({reason})"));
                }
            }
        }

        if let (Some(family), Some(face)) = (primary_declared, fallback.clone()) {
            chain.faces.insert(0, face);

            chain
                .sources
                .insert(0, "LiberationSans-Regular.ttf (bundled)".to_string());

            out.notes.push(format!(
                "chain {index}: {family} is unavailable; Latin text uses the bundled \
                 Liberation Sans instead",
            ));
        }
    }

    if out.chains[1].faces.len() < 2
        && let Ok(bytes) = game.read(b"ui/NanumBarunGothic.ttf")
        && let Ok(face) = face(&bytes)
    {
        out.chains[1].faces.push(face);

        out.chains[1]
            .sources
            .push("ui/NanumBarunGothic.ttf (substituted)".to_string());

        out.notes.push(
            "chain 1: neither Test.ttf nor Gulim.ttc is available; the shipped \
             NanumBarunGothic backs it instead"
                .to_string(),
        );
    }

    out
}
