fn main() {
    cc::Build::new().includes(&[
            "mir",
            "mir/mir-utils",
            "mir/c2mir/",
            "mir/c2mir/x86_64",
            "mir/c2mir/aarch64",
            "mir/c2mir/ppc64",
            "mir/c2mir/s390x",
        ])
        .files(&["mir/mir.c", "mir/c2mir/c2mir.c", "mir/mir-gen.c"])
        .flag_if_supported("-Wall")
        .opt_level(3)
        .compile("mir-sys");

    println!("cargo:rustc-link-lib=mir-sys");
    let bindings = bindgen::builder()
        .detect_include_paths(true)
        .header("mir/mir.h")
        .header("mir/mir-gen.h")
        .header("mir/c2mir/c2mir.h")
        .parse_callbacks(Box::new(bindgen::CargoCallbacks))
        // Finish the builder and generate the bindings.
        .generate()
        // Unwrap the Result and panic on failure.
        .expect("Unable to generate bindings");

    // Write the bindings to the $OUT_DIR/bindings.rs file.
    let out_path = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("bindings.rs"))
        .expect("Couldn't write bindings!");
}
