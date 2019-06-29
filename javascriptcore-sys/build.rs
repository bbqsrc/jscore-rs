use std::env;
use std::path::PathBuf;
use std::process::{Command, Stdio};

// #[cfg(feature = "bundled")]

fn make_bundled() {
    let out_dir = env::var("OUT_DIR").unwrap();
    let here = env::current_dir().unwrap();
    let webkit_library_dir = here.join("WebKit").join("WebKitBuild").join("Release").join("lib");
    let jsc_headers = format!("{}", here.join("WebKit/WebKitBuild/Release/DerivedSources/ForwardingHeaders").display());

    let result = Command::new("Tools/Scripts/build-webkit")
        .current_dir(here.join("WebKit"))
        .args(&["--jsc-only", "--cmakeargs=\"-DENABLE_STATIC_JSC=ON -DUSE_THIN_ARCHIVES=OFF\""])
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .unwrap();

    assert!(result.success());

    let bindings = bindgen::Builder::default()
        .header(format!("{}/JavaScriptCore/JavaScript.h", &jsc_headers))
        .clang_arg("-U__APPLE__")
        .clang_arg(format!("-I{}", &jsc_headers))
        .generate()
        .expect("Unable to generate bindings");

    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("bindings.rs"))
        .expect("Couldn't write bindings!");
    
    println!("cargo:rustc-link-search=native={}", webkit_library_dir.display());
	println!("cargo:rustc-link-lib=static=JavaScriptCore");
	println!("cargo:rustc-link-lib=static=WTF");
	println!("cargo:rustc-link-lib=static=bmalloc");

    if cfg!(not(target_os = "macos")) {
        {
            let lib = pkg_config::find_library("icu-uc").unwrap();
            for library in &lib.libs {
                println!("cargo:rustc-link-lib=dylib={}", library);
            }
        }

        {
            let lib = pkg_config::find_library("icu-i18n").unwrap();
            for library in &lib.libs {
                println!("cargo:rustc-link-lib=dylib={}", library);
            }
        }
	    
        println!("cargo:rustc-link-lib=stdc++");
    } else {
	    println!("cargo:rustc-link-lib=icucore");
	    println!("cargo:rustc-link-lib=c++");
    }
}

fn make_system_macos() {
    let sysroot = "/Applications/Xcode.app/Contents/Developer/Platforms/MacOSX.platform/Developer/SDKs/MacOSX.sdk";

    let bindings = bindgen::Builder::default()
        .header(format!(
            "{}/System/Library/Frameworks/JavaScriptCore.framework/Headers/JavaScriptCore.h",
            sysroot
        ))
        .clang_arg("-U__APPLE__")
        .clang_arg(format!("-isysroot{}", sysroot))
        .clang_arg("-iframework JavaScriptCore")
        .generate()
        .expect("Unable to generate bindings");

    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("bindings.rs"))
        .expect("Couldn't write bindings!");
}

fn main() {
    if cfg!(feature = "bundled") {
        make_bundled()
    } else if cfg!(target_os = "macos") {
        make_system_macos()
    } else {
        panic!("Unsupported build config; try feature `bundled`.")
    }
}