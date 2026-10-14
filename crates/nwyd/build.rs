use std::{path::Path, process::Command};

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
