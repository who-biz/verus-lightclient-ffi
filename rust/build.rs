extern crate cbindgen;

use std::{env, fs, path::PathBuf, process::Command};

fn sdk_and_clang_target(rust_target: &str) -> (Option<&'static str>, Option<&'static str>) {
    match rust_target {
        // iOS device
        "aarch64-apple-ios" => (Some("iphoneos"), Some("arm64-apple-ios")),
        // iOS simulators
        "x86_64-apple-ios" => (Some("iphonesimulator"), Some("x86_64-apple-ios-simulator")),
        "aarch64-apple-ios-sim" => (Some("iphonesimulator"), Some("arm64-apple-ios-simulator")),
        // macOS
        "x86_64-apple-darwin" => (Some("macosx"), Some("x86_64-apple-macos10.12")),
        "aarch64-apple-darwin" => (Some("macosx"), Some("arm64-apple-macos11")),
        // other/unknown
        _ => (None, None),
    }
}

fn sdk_path(sdk: &str) -> String {
    let out = Command::new("xcrun")
        .args(["--sdk", sdk, "--show-sdk-path"])
        .output()
        .expect("xcrun not found; Xcode command-line tools required");
    String::from_utf8(out.stdout)
        .expect("SDK path not UTF-8")
        .trim()
        .to_string()
}

fn main() {
    println!("cargo:rerun-if-changed=wrapper.c");
    println!("cargo:rerun-if-changed=wrapper.h");
    println!("cargo:rerun-if-env-changed=TARGET");
    println!("cargo:rerun-if-env-changed=BINDGEN_EXTRA_CLANG_ARGS");
    println!("cargo:rerun-if-env-changed=BINDGEN_EXTRA_CLANG_ARGS_aarch64-apple-ios-sim");
    println!("cargo:rerun-if-env-changed=BINDGEN_EXTRA_CLANG_ARGS_aarch64_apple_ios_sim");

    let target = env::var("TARGET").unwrap_or_default();
    let (sdk_name_opt, clang_target_opt) = sdk_and_clang_target(&target);

    // Build up common clang args for Apple targets so bindgen/cc see the SDK & correct triple.
    let mut clang_args: Vec<String> = Vec::new();
    if let Some(sdk_name) = sdk_name_opt {
        let sdk = sdk_path(sdk_name);
        clang_args.push("-isysroot".into());
        clang_args.push(sdk.clone());
        // clang_args.push("-fmodules".into());
        // clang_args.push("-fcxx-modules".into());
        // If you need frameworks, uncomment:
        // clang_args.push("-F".into());
        // clang_args.push(format!("{}/System/Library/Frameworks", sdk));

        // commented lines above are optional flags, left here in case we need them at some point
    }
    if let Some(ct) = clang_target_opt {
        clang_args.push("-target".into());
        clang_args.push(ct.into());
    }

    // bindgen stuff
    let mut builder = bindgen::builder()
        .header("wrapper.h")
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        .allowlist_function("os_log_.*")
        .allowlist_function("os_release")
        .allowlist_function("os_signpost_.*");

    for a in &clang_args {
        builder = builder.clang_arg(a);
    }

    let bindings = builder
        .generate()
        .expect("should be able to generate bindings");

    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("bindings.rs"))
        .expect("should be able to write bindings");

    let mut cc_build = cc::Build::new();
    cc_build.file("wrapper.c");
    // Ensure cc sees the same args (SDK, target).
    for a in &clang_args {
        cc_build.flag(a);
    }

    // Include bitcode if it is supported, ignore otherwise
    cc_build.flag_if_supported("-fembed-bitcode");

    // compile shim
    cc_build.compile("wrapper");

    // cbindgen header
    let crate_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let headers_dir = PathBuf::from("target/Headers");
    fs::create_dir_all(&headers_dir).expect("create target/Headers failed");

    if let Ok(b) = cbindgen::Builder::new()
        .with_crate(crate_dir)
        .with_language(cbindgen::Language::C)
        .generate()
    {
        b.write_to_file(headers_dir.join("zcashlc.h"));
    }
}
