fn main() {
    let target = std::env::var("TARGET").unwrap_or_default();
    if !target.contains("-none") {
        return;
    }
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    println!("cargo:rustc-link-arg=-T{}/linker.ld", manifest_dir);
    println!("cargo:rustc-link-arg=-nostdlib");
    println!("cargo:rustc-link-arg=-static");
}
