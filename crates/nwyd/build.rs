use std::{fs, path::Path, process::Command};

fn web_dependencies() {
    if std::env::var("CARGO_CFG_TARGET_ARCH").as_deref() != Ok("wasm32") {
        return;
    }

    println!("cargo:rerun-if-env-changed=NPM");

    let manifest = std::env::var_os("CARGO_MANIFEST_DIR").expect("Cargo manifest directory");
    let web = Path::new(&manifest).join("../../web");
    let modules = web.join("node_modules");
    let stamp = modules.join(".nwyd-build-inputs");
    let vite = modules.join("vite/bin/vite.js");
    let mut inputs = std::env::var("HOST").expect("Cargo host").into_bytes();

    for name in ["package.json", "package-lock.json"] {
        let path = web.join(name);
        println!("cargo:rerun-if-changed={}", path.display());
        inputs.push(0);
        inputs.extend(
            fs::read(&path)
                .unwrap_or_else(|error| panic!("could not read {}: {error}", path.display())),
        );
    }

    for path in [&stamp, &vite] {
        println!("cargo:rerun-if-changed={}", path.display());
    }

    if vite.is_file() && fs::read(&stamp).ok().as_deref() == Some(inputs.as_slice()) {
        return;
    }

    let npm = std::env::var_os("NPM")
        .unwrap_or_else(|| if cfg!(windows) { "npm.cmd" } else { "npm" }.into());
    if stamp.exists() {
        fs::remove_file(&stamp)
            .unwrap_or_else(|error| panic!("could not remove {}: {error}", stamp.display()));
    }
    let status = Command::new(npm)
        .current_dir(&web)
        .args([
            "ci",
            "--include=dev",
            "--ignore-scripts",
            "--no-audit",
            "--no-fund",
            "--prefer-offline",
        ])
        .status()
        .unwrap_or_else(|error| {
            panic!("could not run npm ci: {error}; install Node.js and npm to build for the web")
        });

    assert!(status.success(), "npm ci failed with {status}");
    assert!(vite.is_file(), "npm ci did not install Vite");

    fs::write(&stamp, inputs)
        .unwrap_or_else(|error| panic!("could not write {}: {error}", stamp.display()));
}

fn windows_icon() {
    println!("cargo:rerun-if-changed=resources/windows.rc");

    println!("cargo:rerun-if-changed=resources/windows.manifest");

    println!("cargo:rerun-if-changed=resources/nwyd.ico");

    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }

    #[cfg(not(windows))]
    {
        let target = std::env::var("TARGET").expect("Cargo target");

        let overrides = [
            format!("RC_{target}"),
            format!("RC_{}", target.replace('-', "_")),
            "RC".to_owned(),
        ];

        for name in &overrides {
            println!("cargo:rerun-if-env-changed={name}");
        }

        if overrides
            .iter()
            .all(|name| std::env::var_os(name).is_none())
            && Command::new("llvm-rc").arg("/?").output().is_err()
        {
            let arch = std::env::var("CARGO_CFG_TARGET_ARCH").expect("Cargo target architecture");

            let arch = if arch == "x86" { "i686" } else { &arch };

            let windres = format!("{arch}-w64-mingw32-windres");

            if Command::new(&windres).arg("--version").output().is_ok() {
                unsafe { std::env::set_var("RC", windres) };
            }
        }
    }

    embed_resource::compile_for("resources/windows.rc", ["nwyd"], embed_resource::NONE)
        .manifest_required()
        .expect("compile the nwyd Windows icon resource");
}

fn git(args: &[&str]) -> Option<String> {
    let output = Command::new("git").args(args).output().ok()?;

    output.status.success().then_some(())?;

    Some(String::from_utf8(output.stdout).ok()?.trim().to_owned())
}

fn watch_git_path(name: &str) {
    if let Some(path) = git(&["rev-parse", "--git-path", name]) {
        let mut path = Path::new(&path);

        while !path.exists() {
            let Some(parent) = path.parent() else {
                return;
            };

            path = parent;
        }

        println!("cargo:rerun-if-changed={}", path.display());
    }
}

fn main() {
    println!("cargo:rerun-if-changed=build.rs");

    windows_icon();

    web_dependencies();

    watch_git_path("HEAD");

    watch_git_path("packed-refs");

    if let Some(reference) = git(&["symbolic-ref", "-q", "HEAD"]) {
        watch_git_path(&reference);
    }

    let commit = git(&["rev-parse", "--verify", "HEAD"])
        .filter(|hash| hash.len() >= 8 && hash.bytes().all(|b| b.is_ascii_hexdigit()))
        .map(|hash| hash[..8].to_owned())
        .unwrap_or_else(|| "unknown".to_owned());

    println!("cargo:rustc-env=NWYD_BUILD_COMMIT={commit}");
}
