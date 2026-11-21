#[cfg(feature = "native")]
fn main() {
    if let Err(error) = pfl_desktop::native::run() {
        eprintln!("Patent Forge Lite: {error}");
        std::process::exit(1);
    }
}

#[cfg(not(feature = "native"))]
fn main() {
    eprintln!("Patent Forge Lite desktop requires a native build: cargo run -p pfl-desktop --features native");
    std::process::exit(2);
}
