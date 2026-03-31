fn main() {
    let target = std::env::var("TARGET").unwrap_or_default();
    if target == "xtensa-esp32s3-none-elf" {
        println!("cargo:rustc-link-arg-bins=-Tlinkall.x");
    }
}
