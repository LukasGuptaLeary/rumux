use anyhow::Result;
use clap::CommandFactory;
use clap_complete::{Shell, generate};
use std::io;

pub fn run<C: CommandFactory>(shell: Shell) -> Result<()> {
    let mut cmd = C::command();
    generate(shell, &mut cmd, "rumux", &mut io::stdout());
    Ok(())
}
