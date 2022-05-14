# TransactionProcessor

[![CI](https://github.com/KInterview-MG/transaction-processor/actions/workflows/build.yml/badge.svg)](https://github.com/KInterview-MG/transaction-processor/actions/workflows/build.yml)

Usage:

```bash
$ cargo run -- transactions.csv > accounts.csv
```

## Testing

* A comprehensive set of unit tests (located in each source file) achieve very high coverage.
* The program gets automatically run with sample data located in `test_data` (see `run_with_test_data()` in `main.rs`)

## Error Handling

* Transactions are applied atomically -- either all of each transaction is applied, or none of it.
* If the `TransactionProcessor` detects a transaction error, it will report this through the `Result` return value.
* If the CLI app (`main.rs`) detects a transaction error, it will log the error to `stderr` and skip the transaction.
  * Logging is disabled by default, and can be enabled using the `-v` command line flag.
* No part of the program should ever panic, unless it encounters an allocation failure.
  * E.g. `unwrap()` is only ever used in tests, overflows are always checked and handled gracefully, and collections are accessed safely.

## Design

* Structured in two parts: a library API (`lib.rs`) and the CLI accepting CSV files (`main.rs`).
* Uses the `rust_decimal` crate for currency amounts (see the assumptions section below for the range of values supported)
    * All arithmetic is checked, and a transaction cleanly/atomically fails if it would cause an overflow.
* Transactions are streamed from the CSV file, rather than being loaded all at once.
* Multiple CSV files can be specified, and they will be processed sequentially.

## Additional assumptions

* Locked accounts are prevented from depositing and withdrawing, but allowed to create and handle disputes.
* If a dispute is resolved, a new dispute must be created before a chargeback can occur.
* If a chargeback occurs, the transaction cannot be disputed again.
* Currency amounts are less than `(2^96)/(10^4)` (approx `2^82`). Overflows are handled safely.
* For efficiency, transaction IDs are handled per user account, rather than globally.
  * In other words, two users can both have a deposit/withdrawal with the same ID.
* Duplicate transaction IDs are not allowed within a user's account.