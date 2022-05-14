use std::fmt::{Display, Formatter};
use std::io;

use csv::Trim;
use serde::{Deserialize, Serialize};
use transaction_processor::numeric::CurrencyAmount;
use transaction_processor::{ClientId, Transaction, TransactionId, TransactionType};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "lowercase")]
enum CSVTransactionType {
    Deposit,
    Withdrawal,
    Dispute,
    Resolve,
    Chargeback,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
struct CSVEntry {
    #[serde(rename = "type")]
    transaction_type: CSVTransactionType,
    client: ClientId,
    tx: TransactionId,
    amount: Option<CurrencyAmount>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CSVEntryConvertError {
    MissingAmount,
}

impl Display for CSVEntryConvertError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            CSVEntryConvertError::MissingAmount => "Missing amount",
        })
    }
}

impl TryFrom<CSVEntry> for Transaction {
    type Error = CSVEntryConvertError;

    fn try_from(value: CSVEntry) -> Result<Self, Self::Error> {
        Ok(Self::new(
            value.client,
            value.tx,
            match value.transaction_type {
                CSVTransactionType::Deposit => TransactionType::Deposit {
                    amount: value.amount.ok_or(CSVEntryConvertError::MissingAmount)?,
                },
                CSVTransactionType::Withdrawal => TransactionType::Withdrawal {
                    amount: value.amount.ok_or(CSVEntryConvertError::MissingAmount)?,
                },
                CSVTransactionType::Dispute => TransactionType::Dispute,
                CSVTransactionType::Resolve => TransactionType::Resolve,
                CSVTransactionType::Chargeback => TransactionType::Chargeback,
            },
        ))
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum CSVReaderError {
    CSVParseError(String),
    TransactionParseError(CSVEntryConvertError),
}

impl Display for CSVReaderError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(&match self {
            CSVReaderError::CSVParseError(err) => format!("CSV parse error: {}", err),
            CSVReaderError::TransactionParseError(err) => {
                format!("Transaction parse error: {}", err)
            }
        })
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum CSVWriterError {
    CSVWriteError(String),
}

impl Display for CSVWriterError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(&match self {
            CSVWriterError::CSVWriteError(err) => format!("CSV write error: {}", err),
        })
    }
}

pub struct CSVReader<R: io::Read> {
    reader: csv::Reader<R>,
}

impl<R: io::Read> CSVReader<R> {
    pub fn new(reader: R) -> Self {
        Self {
            reader: csv::ReaderBuilder::new()
                .trim(Trim::All)
                .flexible(true)
                .from_reader(reader),
        }
    }

    pub fn read(&mut self) -> impl Iterator<Item = Result<Transaction, CSVReaderError>> + '_ {
        self.reader.deserialize::<CSVEntry>().map(|entry_result| {
            entry_result
                .map_err(|err| CSVReaderError::CSVParseError(format!("{}", err)))
                .and_then(|t| t.try_into().map_err(CSVReaderError::TransactionParseError))
        })
    }
}

pub struct CSVWriter<W: io::Write> {
    writer: csv::Writer<W>,
}

impl<W: io::Write> CSVWriter<W> {
    pub fn new(writer: W) -> Self {
        Self {
            writer: csv::WriterBuilder::new()
                .has_headers(true)
                .from_writer(writer),
        }
    }

    pub fn write(&mut self, record: impl Serialize) -> Result<(), CSVWriterError> {
        self.writer
            .serialize(record)
            .map_err(|err| CSVWriterError::CSVWriteError(format!("{}", err)))
    }
}

#[cfg(test)]
mod test {
    use std::str::FromStr;

    use transaction_processor::numeric::CurrencyAmount;
    use transaction_processor::{Transaction, TransactionType};

    use crate::csv::{CSVEntryConvertError, CSVReader, CSVReaderError};

    #[test]
    fn test_parse() {
        let data = r###"
            type, client, tx, amount
            deposit, 1, 1, 1.0
            withdrawal, 2, 5, 3.0,
            dispute,7,10
            resolve,8,11
            chargeback,9,12
        "###;

        let mut reader = CSVReader::new(data.as_bytes());
        let mut reader = reader.read();

        assert_eq!(
            Transaction::new(
                1,
                1,
                TransactionType::Deposit {
                    amount: CurrencyAmount::from_str("1.0").unwrap()
                }
            ),
            reader.next().unwrap().unwrap()
        );

        assert_eq!(
            Transaction::new(
                2,
                5,
                TransactionType::Withdrawal {
                    amount: CurrencyAmount::from_str("3.0").unwrap()
                }
            ),
            reader.next().unwrap().unwrap()
        );

        assert_eq!(
            Transaction::new(7, 10, TransactionType::Dispute),
            reader.next().unwrap().unwrap()
        );

        assert_eq!(
            Transaction::new(8, 11, TransactionType::Resolve),
            reader.next().unwrap().unwrap()
        );

        assert_eq!(
            Transaction::new(9, 12, TransactionType::Chargeback),
            reader.next().unwrap().unwrap()
        );
    }

    #[test]
    fn test_parse_fail() {
        let data = r###"
            type, client, tx, amount
            deposit, 1, 1, 1.0
            unknown, 2, 5, 3.0,
            dispute,7,10
            deposit,1,1
            chargeback,9,12
        "###;

        let mut reader = CSVReader::new(data.as_bytes());
        let mut reader = reader.read();

        assert_eq!(
            Transaction::new(
                1,
                1,
                TransactionType::Deposit {
                    amount: CurrencyAmount::from_str("1.0").unwrap()
                }
            ),
            reader.next().unwrap().unwrap()
        );

        assert!(matches!(
            reader.next().unwrap(),
            Err(CSVReaderError::CSVParseError(_))
        ));

        assert_eq!(
            Transaction::new(7, 10, TransactionType::Dispute),
            reader.next().unwrap().unwrap()
        );

        assert_eq!(
            Err(CSVReaderError::TransactionParseError(
                CSVEntryConvertError::MissingAmount
            )),
            reader.next().unwrap()
        );

        assert_eq!(
            Transaction::new(9, 12, TransactionType::Chargeback),
            reader.next().unwrap().unwrap()
        );
    }
}
