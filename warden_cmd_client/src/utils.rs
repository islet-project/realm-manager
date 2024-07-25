use std::io;
use std::io::Write;

use clap::Parser;

use crate::cmd_parser::CmdParser;

pub fn read_command_line() -> Result<CmdParser, anyhow::Error> {
    write_line_begining()?;
    let mut line = String::new();
    io::stdin().read_line(&mut line)?;
    let argv = shlex::split(line.trim().as_ref()).ok_or(io::Error::other("Can't split readed line!"))?;
    let cmd = CmdParser::try_parse_from(argv.iter())?;
    Ok(cmd)
}

fn write_line_begining() -> Result<(), anyhow::Error> {
    write!(std::io::stdout(), "$ ")?;
    Ok(std::io::stdout().flush()?)
}