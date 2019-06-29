use std::env;
use std::path::PathBuf;

fn main() {
    println!("cargo:rustc-link-lib=framework=JavaScriptCore");
    let sysroot = "/Applications/Xcode.app/Contents/Developer/Platforms/MacOSX.platform/Developer/SDKs/MacOSX.sdk";

    let bindings = bindgen::Builder::default()
        .header(format!(
            "{}/System/Library/Frameworks/JavaScriptCore.framework/Headers/JavaScriptCore.h",
            sysroot
        ))
        .clang_arg(format!("-isysroot{}", sysroot))
        .clang_arg("-iframework JavaScriptCore")
        .generate()
        .expect("Unable to generate bindings");

    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("bindings.rs"))
        .expect("Couldn't write bindings!");
}
