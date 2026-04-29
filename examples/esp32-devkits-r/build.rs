fn main() {
    println!("cargo:rustc-link-arg=-Wl,-Tlinkall.x");
    println!("cargo:rustc-link-arg=-nostartfiles");
}
