#!/bin/bash

set -e

# Most of the testing is done using "cargo test", however this
# script just verifies that running the app with the command
# specified in the spec actually works correctly.

cp test_data/002_input.csv transactions.csv

cargo run -- transactions.csv > accounts.csv

cmp --silent accounts.csv test_data/002_expected.csv

rm transactions.csv accounts.csv