use anyhow::Result;
use console::style;

pub fn run() -> Result<()> {
    println!("rumux {}", env!("CARGO_PKG_VERSION"));
    println!();
    println!("To install or update the latest release, run:");
    println!(
        "  {}",
        style("curl -fsSL https://raw.githubusercontent.com/LukasGuptaLeary/rumux/main/install.sh | sh")
            .green()
    );
    println!();
    println!("To update rumux from a source checkout, run:");
    println!(
        "  {}",
        style("cargo install --git https://github.com/LukasGuptaLeary/rumux.git rumux-cli --bin rumux --force")
            .green()
    );
    Ok(())
}
