use std::io;
use std::io::Write;

use clap::Parser;

use crate::cmd_parser::CmdParser;

pub fn parse_users_input() -> Result<CmdParser, anyhow::Error> {
    write_line_begining()?;
    let line = read_line()?;
    let argv =
        shlex::split(line.trim()).ok_or(io::Error::other("Can't split readed line!"))?;
    Ok(CmdParser::try_parse_from(argv.iter())?)
}

fn write_line_begining() -> Result<(), anyhow::Error> {
    write!(std::io::stdout(), "$ ")?;
    Ok(std::io::stdout().flush()?)
}

fn read_line() -> Result<String, anyhow::Error> {
    let mut line = String::new();
    io::stdin().read_line(&mut line)?;
    Ok(line)
}