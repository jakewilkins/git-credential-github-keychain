fn main() {
    // Lets `update` pick the matching release asset (arm vs armv7 share target_arch = "arm").
    println!("cargo:rustc-env=BUILD_TARGET={}", std::env::var("TARGET").unwrap());
    println!("cargo:rerun-if-changed=build.rs");
}
