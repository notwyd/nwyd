use flate2::read::ZlibDecoder;
use sha2::{Digest, Sha256};
use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

#[cfg(test)]
mod tests;

struct Bundle {
    id: &'static str,
    files: &'static [BundleFile],
}

struct BundleFile {
    path: &'static str,
    compressed: &'static [u8],
    sha256: [u8; 32],
    uncompressed_len: u64,
}

#[cfg(windows)]
include!(concat!(env!("OUT_DIR"), "/neural_bundle.rs"));

#[cfg(windows)]
pub(super) fn directory() -> Result<Option<PathBuf>, String> {
    let Some(bundle) = BUNDLE.as_ref() else {
        return Ok(None);
    };
    let root = dirs::data_local_dir()
        .ok_or("Cannot locate the local application-data directory")?
        .join("nwyd")
        .join("neural");
    install(&root, bundle)
        .map(Some)
        .map_err(|error| format!("Preparing bundled neural runtime: {error}"))
}

fn selected_files(bundle: &Bundle) -> io::Result<Vec<(&BundleFile, &str)>> {
    if bundle.id.len() != 64 || !bundle.id.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(io::Error::other("Invalid neural bundle identifier"));
    }
    let mut selected = Vec::new();
    for file in bundle.files {
        let name = match file.path {
            "nwyd_neural.dll"
            | "nvngx_dlssnr.dll"
            | "LICENSE-neural-runtime.txt"
            | "NOTICE-neural-runtime.txt"
            | "ATTRIBUTION-DLSS5VKLayer.txt"
            | "LICENSE-nvidia-runtime.txt" => file.path,
            _ => return Err(io::Error::other("Invalid neural bundle filename")),
        };
        if selected.iter().any(|(_, prior)| *prior == name) {
            return Err(io::Error::other("Duplicate neural bundle filename"));
        }
        selected.push((file, name));
    }
    for name in ["nwyd_neural.dll", "nvngx_dlssnr.dll"] {
        if !selected.iter().any(|(_, actual)| *actual == name) {
            return Err(io::Error::other("Incomplete neural bundle"));
        }
    }
    Ok(selected)
}

fn valid_cache(path: &Path, files: &[(&BundleFile, &str)]) -> bool {
    if !fs::symlink_metadata(path).is_ok_and(|metadata| metadata.is_dir()) {
        return false;
    }
    for forbidden in ["nvngx.dll", "_nvngx.dll", "nvapi64.dll", "dxgi.dll"] {
        if fs::symlink_metadata(path.join(forbidden)).is_ok() {
            return false;
        }
    }
    files.iter().all(|(expected, name)| {
        let path = path.join(name);
        if !fs::symlink_metadata(&path)
            .is_ok_and(|metadata| metadata.is_file() && metadata.len() == expected.uncompressed_len)
        {
            return false;
        }
        let Ok(mut file) = File::open(path) else {
            return false;
        };
        let mut hash = Sha256::new();
        let mut buffer = [0; 64 * 1024];
        loop {
            match file.read(&mut buffer) {
                Ok(0) => return <[u8; 32]>::from(hash.finalize()) == expected.sha256,
                Ok(size) => hash.update(&buffer[..size]),
                Err(_) => return false,
            }
        }
    })
}

struct TemporaryDirectory(PathBuf);

impl Drop for TemporaryDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn temporary_directory(parent: &Path) -> io::Result<TemporaryDirectory> {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    loop {
        let path = parent.join(format!(
            ".extract-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        match fs::create_dir(&path) {
            Ok(()) => return Ok(TemporaryDirectory(path)),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
    }
}

fn extract(file: &BundleFile, path: &Path) -> io::Result<()> {
    let mut decoder =
        ZlibDecoder::new(file.compressed).take(file.uncompressed_len.saturating_add(1));
    let mut output = File::options().write(true).create_new(true).open(path)?;
    let mut hash = Sha256::new();
    let mut buffer = [0; 64 * 1024];
    let mut length = 0;
    loop {
        let size = decoder.read(&mut buffer)?;
        if size == 0 {
            break;
        }
        length += size as u64;
        if length > file.uncompressed_len {
            return Err(io::Error::other("Neural bundle payload exceeds its size"));
        }
        hash.update(&buffer[..size]);
        output.write_all(&buffer[..size])?;
    }
    if length != file.uncompressed_len || <[u8; 32]>::from(hash.finalize()) != file.sha256 {
        return Err(io::Error::other(
            "Neural bundle payload failed verification",
        ));
    }
    output.sync_all()
}

fn install(root: &Path, bundle: &Bundle) -> io::Result<PathBuf> {
    let files = selected_files(bundle)?;
    let parent = root.join(bundle.id);
    let target = parent.join("native");
    fs::create_dir_all(&parent)?;
    let lock = File::options()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(parent.join("native.lock"))?;
    lock.lock()?;
    if valid_cache(&target, &files) {
        return Ok(target);
    }
    let staging = temporary_directory(&parent)?;
    for (file, name) in &files {
        extract(file, &staging.0.join(name))?;
    }
    publish(&staging.0, &target)?;
    Ok(target)
}

fn publish(staging: &Path, target: &Path) -> io::Result<()> {
    let retired = if fs::symlink_metadata(target).is_ok() {
        let retired = temporary_directory(target.parent().unwrap())?;
        fs::rename(target, retired.0.join("old"))?;
        Some(retired)
    } else {
        None
    };
    if let Err(error) = fs::rename(staging, target) {
        if let Some(retired) = retired {
            if fs::rename(retired.0.join("old"), target).is_err() {
                let retained = retired.0.display().to_string();
                std::mem::forget(retired);
                return Err(io::Error::other(format!(
                    "Publishing neural runtime: {error}; previous cache retained at {retained}"
                )));
            }
        }
        return Err(error);
    }
    Ok(())
}
