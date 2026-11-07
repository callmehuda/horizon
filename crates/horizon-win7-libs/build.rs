//! The win7 targets and the old `windows-targets`: `windows_i686_msvc` 0.42 and 0.48 (still
//! under gpui, and under some of the game's older crates) tell the linker where their import
//! libraries are only for the `pc` and `uwp` triples — their build scripts return early for
//! any other, and a `*-win7-windows-msvc` link then fails with
//! `LNK1181: cannot open input file 'windows.0.48.5.lib'`. The libraries themselves work for
//! the win7 triples as they are (the same names, the same functions, another baseline), so
//! for those targets this adds their `.lib` folders to the linker's search path, straight
//! from the cargo home where the crates already sit.
//!
//! (0.52 and newer emit their search path for every target; only the old ones need this.)

fn main() {
    if std::env::var("CARGO_CFG_TARGET_VENDOR").as_deref() != Ok("win7") {
        return;
    }
    let arch = match std::env::var("CARGO_CFG_TARGET_ARCH").as_deref() {
        Ok("x86_64") => "x86_64",
        Ok("x86") => "i686",
        Ok("aarch64") => "aarch64",
        _ => return,
    };
    // The cargo home holds the registry: registry/src/<registry>/windows_<arch>_msvc-<version>/lib
    let home = std::env::var("CARGO_HOME").unwrap_or_else(|_| {
        let home = std::env::var("HOME").unwrap_or_default();
        format!("{home}/.cargo")
    });
    let src = std::path::Path::new(&home).join("registry/src");
    let wanted = format!("windows_{arch}_msvc-");
    let mut found = 0;
    if let Ok(registries) = std::fs::read_dir(&src) {
        for registry in registries.flatten() {
            let Ok(crates) = std::fs::read_dir(registry.path()) else {
                continue;
            };
            for krate in crates.flatten() {
                if !krate
                    .file_name()
                    .to_string_lossy()
                    .starts_with(&wanted)
                {
                    continue;
                }
                let lib = krate.path().join("lib");
                if lib.is_dir() {
                    println!("cargo:rustc-link-search=native={}", lib.display());
                    found += 1;
                }
            }
        }
    }
    if found == 0 {
        println!(
            "cargo:warning=horizon-win7-libs: no {wanted} import libraries under {src:?} \
             (is CARGO_HOME right?); the win7 link will fail without them"
        );
    }
}
