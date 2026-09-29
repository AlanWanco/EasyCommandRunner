fn main() {
    let resources = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../resources");
    let rc = resources.join("app_icon.rc");
    let icon = resources.join("app_icon.ico");
    println!("cargo:rerun-if-changed={}", rc.display());
    println!("cargo:rerun-if-changed={}", icon.display());
    #[cfg(windows)]
    {
        // The GPUI window and its deeply nested views are created on the Windows
        // process's main thread. The PE default reserves only 1 MiB, which
        // overflows before the window appears; reserve 8 MiB for that thread.
        println!("cargo:rustc-link-arg-bin=easy-command-runner-gpui-prototype=/STACK:8388608");
        // The .rc file references app_icon.ico by name. Include its directory so
        // MSVC's resource compiler can resolve it regardless of Cargo's cwd.
        embed_resource::compile_for(
            &rc,
            ["easy-command-runner-gpui-prototype"],
            embed_resource::ParamsIncludeDirs([resources]),
        )
        .manifest_required()
        .unwrap();
    }
}
