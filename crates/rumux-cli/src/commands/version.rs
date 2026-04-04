use anyhow::Result;

pub fn run() -> Result<()> {
    println!("rumux {}", env!("CARGO_PKG_VERSION"));
    Ok(())
}
