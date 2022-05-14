//! Command line interface for the transaction processor.
//!
//! Makes use of the API in `lib.rs`.

#![deny(missing_docs)]

use std::fmt::{Display, Formatter};
use std::fs::File;
use std::io;

use log::LevelFilter;
use transaction_processor::TransactionProcessor;

use crate::args::{parse_args, ArgsError};
use crate::csv::{CSVReader, CSVWriter};

mod args;
mod csv;

fn main() {
    if let Err(err) = env_logger::Builder::new()
        .filter_level(LevelFilter::Info)
        .try_init()
    {
        eprintln!("Failed to create logger ({}). Continuing anyway.", err);
    }

    if let Err(err) = transaction_processor_cli() {
        log::error!("{}", err);
        std::process::exit(1);
    }
}

/// Parses the command line arguments and runs the transaction processor.
fn transaction_processor_cli() -> Result<(), TransactionProcessorCLIError> {
    let args = parse_args()?;
    process_files(args.input_files(), io::stdout())
}

/// Processes the list of transactions in the specified files, and outputs
/// a CSV report to the specified writer.
fn process_files(
    input_files: &[String],
    output: impl io::Write,
) -> Result<(), TransactionProcessorCLIError> {
    let mut transaction_processor = TransactionProcessor::new();

    for arg in input_files {
        log::info!("Reading file {}", arg);

        let mut csv_reader = CSVReader::new(File::open(arg).map_err(|error| {
            TransactionProcessorCLIError::FailedToOpenFile {
                path: arg.clone(),
                error,
            }
        })?);

        for transaction in csv_reader.read() {
            // In a production banking system, it would make sense to
            // take more drastic action here if an error occurs. This may
            // include, for example, storing the failed transaction
            // somewhere for human inspection and resolution.
            match transaction {
                Ok(transaction) => {
                    if let Err(err) = transaction_processor.transact(&transaction) {
                        log::error!("Got error '{}' processing transaction. Skipping.", err);
                    }
                }
                Err(err) => {
                    log::error!("Got error '{}' reading CSV. Skipping transaction.", err);
                }
            }
        }
    }

    let mut writer = CSVWriter::new(output);

    for entry in transaction_processor.generate_report() {
        if let Err(err) = writer.write(entry) {
            log::error!("Failed to write entry: {}", err);
        }
    }

    Ok(())
}

/// Fatal error occurred when running the application.
#[derive(Debug)]
enum TransactionProcessorCLIError {
    /// There was a problem with the provided command line arguments.
    ArgsError(ArgsError),
    /// One of the specified files could not be opened.
    FailedToOpenFile { path: String, error: io::Error },
}

impl Display for TransactionProcessorCLIError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(&match self {
            TransactionProcessorCLIError::ArgsError(err) => {
                format!("Invalid arguments: {}", err)
            }
            TransactionProcessorCLIError::FailedToOpenFile { path, error } => {
                format!("Failed to open '{}': {}", path, error)
            }
        })
    }
}

impl From<ArgsError> for TransactionProcessorCLIError {
    fn from(err: ArgsError) -> Self {
        Self::ArgsError(err)
    }
}

#[cfg(test)]
mod test {
    use log::LevelFilter;

    use crate::process_files;

    #[test]
    fn run_with_test_data() {
        if let Err(err) = env_logger::Builder::new()
            .filter_level(LevelFilter::Info)
            .try_init()
        {
            eprintln!("Failed to create logger ({}). Continuing anyway.", err);
        }

        for file in 0..=7 {
            let input_file = format!("test_data/{:03}_input.csv", file);
            let expected_output_file = format!("test_data/{:03}_expected.csv", file);

            let mut output = Vec::new();
            process_files(&[input_file], &mut output).unwrap();

            let expected = std::fs::read_to_string(expected_output_file)
                .unwrap()
                .replace("\r\n", "\n");

            assert_eq!(expected, String::from_utf8_lossy(output.as_slice()));
        }
    }
}
