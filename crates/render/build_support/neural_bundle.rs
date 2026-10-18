use flate2::Compression;
use flate2::write::ZlibEncoder;
use sha2::{Digest, Sha256};
use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::Path;

#[cfg(test)]
#[path = "neural_bundle/tests.rs"]
mod tests;

const REQUIRED: [&str; 5] = [
    "nwyd_neural.dll",
    "nvngx_dlssnr.dll",
    "LICENSE-neural-runtime.txt",
    "NOTICE-neural-runtime.txt",
    "ATTRIBUTION-DLSS5VKLayer.txt",
];

const OPTIONAL: [&str; 1] = ["LICENSE-nvidia-runtime.txt"];

pub fn build(source: &Path, output: &Path) -> Result<(), String> {
    if !source.is_dir() {
        return Err(format!(
            "{} is not a runtime staging directory",
            source.display()
        ));
    }

    println!("cargo:rerun-if-changed={}", source.display());

    let names: Vec<_> = REQUIRED
        .into_iter()
        .chain(
            OPTIONAL
                .into_iter()
                .filter(|name| source.join(name).exists()),
        )
        .collect();

    fs::create_dir_all(output).map_err(|error| format!("creating output directory: {error}"))?;

    let mut bundle_hash = Sha256::new();

    bundle_hash.update(b"NWYD neural runtime bundle v1\0zlib\0");

    let mut declarations = String::new();

    for (index, name) in names.into_iter().enumerate() {
        let path = source.join(name);

        println!("cargo:rerun-if-changed={}", path.display());

        let bytes =
            fs::read(&path).map_err(|error| format!("reading required payload {name}: {error}"))?;

        if bytes.is_empty() {
            return Err(format!("payload {name} is empty"));
        }

        if name.ends_with(".dll") {
            validate_dll(&bytes).map_err(|error| format!("invalid DLL {name}: {error}"))?;
        } else if std::str::from_utf8(&bytes).is_err() {
            return Err(format!(
                "attribution document {name} must contain UTF-8 text"
            ));
        }

        let sha256: [u8; 32] = Sha256::digest(&bytes).into();

        let uncompressed_len = bytes.len() as u64;

        bundle_hash.update((name.len() as u32).to_le_bytes());
        bundle_hash.update(name.as_bytes());
        bundle_hash.update(uncompressed_len.to_le_bytes());
        bundle_hash.update(sha256);

        let compressed_name = format!("neural_bundle_{index}.zlib");

        let compressed_path = output.join(&compressed_name);

        let file = File::create(&compressed_path)
            .map_err(|error| format!("creating compressed payload {name}: {error}"))?;

        let mut encoder = ZlibEncoder::new(BufWriter::new(file), Compression::default());

        encoder
            .write_all(&bytes)
            .map_err(|error| format!("compressing {name}: {error}"))?;

        encoder
            .finish()
            .and_then(|mut writer| writer.flush())
            .map_err(|error| format!("writing compressed payload {name}: {error}"))?;

        declarations.push_str(&format!(
            "    BundleFile {{ path: {name:?}, compressed: include_bytes!(concat!(env!(\"OUT_DIR\"), \"/{compressed_name}\")), sha256: {sha256:?}, uncompressed_len: {uncompressed_len} }},\n"
        ));
    }

    let id = format!("{:x}", bundle_hash.finalize());

    let declaration = format!(
        "const BUNDLE: Option<Bundle> = Some(Bundle {{ id: {id:?}, files: &[\n{declarations}] }});\n"
    );

    fs::write(output.join("neural_bundle.rs"), declaration)
        .map_err(|error| format!("writing the bundle declaration: {error}"))
}

fn validate_dll(bytes: &[u8]) -> Result<(), &'static str> {
    let word = |offset| {
        bytes
            .get(offset..offset + 2)
            .map(|value| u16::from_le_bytes(value.try_into().unwrap()))
    };

    let dword = |offset| {
        bytes
            .get(offset..offset + 4)
            .map(|value| u32::from_le_bytes(value.try_into().unwrap()))
    };

    if bytes.get(..2) != Some(b"MZ") {
        return Err("missing DOS MZ signature");
    }

    let pe = usize::try_from(dword(0x3c).ok_or("truncated DOS header")?)
        .map_err(|_| "PE offset exceeds the address space")?;

    if pe < 64 || pe.checked_add(24).is_none_or(|end| end > bytes.len()) {
        return Err("PE header offset is outside the payload");
    }

    if bytes.get(pe..pe + 4) != Some(b"PE\0\0") {
        return Err("missing PE signature");
    }

    if word(pe + 4) != Some(0x8664) {
        return Err("the PE machine is not AMD64");
    }

    let sections = usize::from(word(pe + 6).ok_or("truncated COFF header")?);

    if sections == 0 {
        return Err("the PE section table is empty");
    }

    if word(pe + 22).ok_or("truncated COFF flags")? & 0x2000 == 0 {
        return Err("the PE image is not marked as a DLL");
    }

    let optional_size = usize::from(word(pe + 20).ok_or("truncated optional-header size")?);

    if optional_size < 112 || word(pe + 24) != Some(0x20b) {
        return Err("the PE optional header is not PE32+");
    }

    let section_start = (pe + 24)
        .checked_add(optional_size)
        .ok_or("PE header size overflow")?;

    let section_end = section_start
        .checked_add(sections * 40)
        .ok_or("PE section table size overflow")?;

    if section_end > bytes.len() {
        return Err("truncated PE optional header or section table");
    }

    for section in (section_start..section_end).step_by(40) {
        let length = usize::try_from(dword(section + 16).ok_or("truncated PE section")?)
            .map_err(|_| "PE section size exceeds the address space")?;

        let offset = usize::try_from(dword(section + 20).ok_or("truncated PE section")?)
            .map_err(|_| "PE section offset exceeds the address space")?;

        if length != 0
            && offset
                .checked_add(length)
                .is_none_or(|end| end > bytes.len())
        {
            return Err("PE section contents exceed the payload");
        }
    }

    Ok(())
}
