#![allow(clippy::module_name_repetitions)]

use std::fmt::{Display, Formatter};

use clap::{arg, Command};

/// Command line arguments for the CLI interface.
pub struct Args {
    input_files: Vec<String>,
}

impl Args {
    /// The list of CSV input files specified.
    pub fn input_files(&self) -> &[String] {
        self.input_files.as_slice()
    }
}

#[derive(Clone, Debug)]
pub enum ArgsError {
    NoInputFilesSpecified,
}

impl Display for ArgsError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            ArgsError::NoInputFilesSpecified => "No input files specified",
        })
    }
}

pub fn parse_args() -> Result<Args, ArgsError> {
    let arg_matches = Command::new("transaction-processor")
        .trailing_var_arg(true)
        .arg(arg!(<input> ... "input csv file"))
        .get_matches();

    let input_files: Vec<_> = arg_matches
        .values_of("input")
        .ok_or(ArgsError::NoInputFilesSpecified)?
        .collect();

    Ok(Args {
        input_files: input_files
            .iter()
            .map(|input_file| (*input_file).to_string())
            .collect(),
    })
}
