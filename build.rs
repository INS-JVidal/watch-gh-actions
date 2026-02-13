fn main() {
    let build_number = std::fs::read_to_string("BUILD_NUMBER")
        .unwrap_or_else(|_| "0".to_string())
        .trim()
        .to_string();

    println!("cargo:rustc-env=BUILD_NUMBER={}", build_number);
    println!("cargo:rerun-if-changed=BUILD_NUMBER");
}
