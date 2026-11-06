fn main() {
    println!("cargo:rerun-if-changed=build.rs");

    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let target_env = std::env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();
    if target_os == "windows" && target_env == "msvc" {
        let manifest_path = std::path::Path::new(&std::env::var("CARGO_MANIFEST_DIR").unwrap())
            .join("windows")
            .join("rabbit.exe.manifest");
        println!("cargo:rerun-if-changed={}", manifest_path.display());
        println!("cargo:rustc-link-arg-bin=rabbit=/MANIFEST:EMBED");
        println!(
            "cargo:rustc-link-arg-bin=rabbit=/MANIFESTINPUT:{}",
            manifest_path.display()
        );
        // The DLLs rabbit.exe imports come from System32 only
        // (LOAD_LIBRARY_SEARCH_SYSTEM32), never from the folder rabbit.exe
        // sits in. RABBIT is portable and often runs from Downloads, a
        // folder anyone in the user's session can write to, and its
        // elevated uninstall helper is rabbit.exe again: without this, a
        // version.dll or VCRUNTIME140.dll put next to it would run with
        // administrator rights before main() is reached, behind a
        // permission prompt that shows RABBIT's own signature. The flag
        // covers every import, delay-loaded ones too; it is not inherited
        // by the DLLs those load. tests/dll_search.rs checks it is set.
        println!("cargo:rustc-link-arg-bin=rabbit=/DEPENDENTLOADFLAG:0x800");
    }
}
