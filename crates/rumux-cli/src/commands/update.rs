use anyhow::Result;
use console::style;

pub fn run() -> Result<()> {
    println!("rumux {}", env!("CARGO_PKG_VERSION"));
    println!();
    println!("To update rumux from a source checkout, run:");
    println!(
        "  {}",
        style("cargo install --path crates/rumux-cli --force").green()
    );
    Ok(())
}
