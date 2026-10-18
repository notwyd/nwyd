use std::env;
use std::fs;
use std::path::PathBuf;

#[cfg(feature = "bundled-neural-runtime")]
#[path = "build_support/neural_bundle.rs"]
mod neural_bundle;

fn main() {
    println!("cargo:rerun-if-env-changed=NWYD_NEURAL_BUNDLE_DIR");
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=build_support/neural_bundle.rs");
    println!("cargo:rerun-if-changed=build_support/neural_bundle/tests.rs");

    let output = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo must set OUT_DIR"));

    fs::write(
        output.join("neural_bundle.rs"),
        "const BUNDLE: Option<Bundle> = None;\n",
    )
    .expect("could not write the neural runtime bundle declaration");

    #[cfg(feature = "bundled-neural-runtime")]
    if env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        if env::var("CARGO_CFG_TARGET_ARCH").as_deref() != Ok("x86_64") {
            panic!("bundled-neural-runtime requires an x86_64 Windows target");
        }

        let source = env::var_os("NWYD_NEURAL_BUNDLE_DIR")
            .filter(|path| !path.is_empty())
            .map(PathBuf::from)
            .expect("bundled-neural-runtime requires NWYD_NEURAL_BUNDLE_DIR to name the runtime staging directory");

        neural_bundle::build(&source, &output)
            .unwrap_or_else(|error| panic!("could not bundle the neural runtime: {error}"));
    }
}
