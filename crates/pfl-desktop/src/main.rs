#[cfg(feature = "native")]
fn main() {
    if let Err(error) = pfl_desktop::native::run() {
        eprintln!("Patent Forge Lite: {error}");
        // A second instance is refused on purpose, with its own exit code so a
        // launcher can tell it apart from a real failure.
        let message = error.to_string();
        let code = if message.contains("only one instance may write") {
            3
        } else {
            1
        };
        std::process::exit(code);
    }
}

#[cfg(not(feature = "native"))]
fn main() {
    eprintln!(
        "Patent Forge Lite desktop requires a native build: cargo run -p pfl-desktop --features native"
    );
    std::process::exit(2);
}
