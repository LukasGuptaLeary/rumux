use anyhow::Result;
use console::style;

pub fn run() -> Result<()> {
    println!("rumux {}", env!("CARGO_PKG_VERSION"));
    println!();
    println!("To update rumux, run:");
    println!("  {}", style("cargo install rumux").green());
    println!();
    println!("For local development:");
    println!("  {}", style("cargo install --path .").green());
    Ok(())
}
