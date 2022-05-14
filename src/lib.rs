//! Transaction processor library -- exposes an API which is used by the
//! CLI frontend.
//!
//! See README.md for more details.

#![deny(missing_docs)]

use std::collections::hash_map::Entry;
use std::collections::{btree_map, BTreeMap, HashMap, HashSet};
use std::fmt::{Display, Formatter};

use serde::Serialize;

use crate::numeric::{CurrencyAmount, CurrencyError};

/// Numeric module: contains currency-related types.
pub mod numeric;

/// Error returned when a transaction could not be applied to an account.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TransactionError {
    /// The specified transaction does not exist for this user.
    TransactionDoesNotExist(TransactionId),
    /// Cannot create a transaction with this ID, as it already exists
    /// for this user.
    TransactionAlreadyExists(TransactionId),
    /// This transaction is already disputed for this user.
    DisputeAlreadyExists(TransactionId),
    /// This dispute cannot be resolved as the transaction is not disputed.
    DisputeDoesNotExist(TransactionId),
    /// An arithmetic error occurred (overflow/underflow) when calculating the
    /// account balances.
    CurrencyError(CurrencyError),
    /// This account is locked and cannot deposit/withdraw money.
    AccountIsLocked,
    /// This withdrawal would take the account balance below zero.
    NotEnoughFunds,
}

impl Display for TransactionError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(&match self {
            TransactionError::TransactionDoesNotExist(tx) => {
                format!("Transaction {} does not exist", tx)
            }
            TransactionError::TransactionAlreadyExists(tx) => {
                format!("Transaction {} already exists", tx)
            }
            TransactionError::DisputeAlreadyExists(tx) => {
                format!("Dispute {} already exists", tx)
            }
            TransactionError::DisputeDoesNotExist(tx) => {
                format!("Dispute {} does not exist", tx)
            }
            TransactionError::CurrencyError(err) => {
                format!("Currency error: {}", err)
            }
            TransactionError::AccountIsLocked => "Account is locked".to_string(),
            TransactionError::NotEnoughFunds => "Not enough funds".to_string(),
        })
    }
}

impl From<CurrencyError> for TransactionError {
    fn from(err: CurrencyError) -> Self {
        Self::CurrencyError(err)
    }
}

#[derive(Clone, Copy, Debug)]
enum DisputeResolution {
    Resolve,
    Chargeback,
}

struct ClientAccount {
    available: CurrencyAmount,
    held: CurrencyAmount,
    /// Positive CurrencyAmount for a deposit, negative for a withdrawal
    transactions: HashMap<TransactionId, CurrencyAmount>,
    active_disputes: HashSet<TransactionId>,
    locked: bool,
}

impl ClientAccount {
    pub fn new() -> Self {
        Self {
            available: CurrencyAmount::ZERO,
            held: CurrencyAmount::ZERO,
            transactions: HashMap::new(),
            active_disputes: HashSet::new(),
            locked: false,
        }
    }

    pub fn total(&self) -> Result<CurrencyAmount, CurrencyError> {
        self.available + self.held
    }

    /// Disputes the specified transaction in the user's account. All changes
    /// occur atomically.
    ///
    /// This will transfer the value of the transaction from the available
    /// funds to the held funds, and mark the transaction as disputed.
    fn create_dispute(&mut self, tx: TransactionId) -> Result<(), TransactionError> {
        let amount = self
            .transactions
            .get(&tx)
            .ok_or(TransactionError::TransactionDoesNotExist(tx))?;

        // Update these atomically in case of an error
        let new_held = (self.held + *amount)?;
        let new_available = (self.available - *amount)?;

        if !self.active_disputes.insert(tx) {
            return Err(TransactionError::DisputeAlreadyExists(tx));
        }

        self.held = new_held;
        self.available = new_available;

        Ok(())
    }

    /// Resolves an existing dispute in the specified manner. The transaction
    /// must already be marked as disputed.
    fn resolve_dispute(
        &mut self,
        tx: TransactionId,
        resolution: DisputeResolution,
    ) -> Result<(), TransactionError> {
        let amount = self
            .transactions
            .get(&tx)
            .ok_or(TransactionError::TransactionDoesNotExist(tx))?;

        let new_held = (self.held - *amount)?;

        let new_available = match resolution {
            DisputeResolution::Resolve => (self.available + *amount)?,
            DisputeResolution::Chargeback => self.available,
        };

        if !self.active_disputes.remove(&tx) {
            return Err(TransactionError::DisputeDoesNotExist(tx));
        }

        if matches!(resolution, DisputeResolution::Chargeback) {
            // Ensure that this transaction cannot be disputed again
            self.transactions.remove(&tx);
            self.locked = true;
        }

        self.held = new_held;
        self.available = new_available;

        Ok(())
    }

    /// Increases the available funds by the specified amount.
    fn deposit(
        &mut self,
        tx: TransactionId,
        amount: CurrencyAmount,
    ) -> Result<(), TransactionError> {
        if self.locked {
            return Err(TransactionError::AccountIsLocked);
        }

        let new_available = (self.available + amount)?;

        if new_available.is_negative() {
            return Err(TransactionError::NotEnoughFunds);
        }

        match self.transactions.entry(tx) {
            Entry::Occupied(_) => return Err(TransactionError::TransactionAlreadyExists(tx)),
            Entry::Vacant(entry) => {
                entry.insert(amount);
            }
        }

        self.available = new_available;

        Ok(())
    }

    /// Reduces the available funds by the specified amount.
    fn withdraw(
        &mut self,
        tx: TransactionId,
        amount: CurrencyAmount,
    ) -> Result<(), TransactionError> {
        self.deposit(tx, -amount)
    }
}

/// A description of a specific client account in a generated report.
#[derive(Serialize, Clone, Debug, Eq, PartialEq)]
pub struct ReportEntry {
    /// The ID of the client.
    client: ClientId,
    /// The amount of available funds.
    available: CurrencyAmount,
    /// The amount of held (i.e. disputed) funds.
    held: CurrencyAmount,
    /// The sum of the available and held funds.
    total: CurrencyAmount,
    /// Whether the account is locked.
    locked: bool,
}

/// Transaction processor main struct. Processes a stream of transactions
/// provided using [`TransactionProcessor::transact`], and then generates
/// a report on the final state of all accounts using
/// [`TransactionProcessor::generate_report`]
pub struct TransactionProcessor {
    // Store in ClientId order (to make testing/comparing output easier)
    clients: BTreeMap<ClientId, ClientAccount>,
}

impl TransactionProcessor {
    /// Creates a new instance of [`TransactionProcessor`] with no client
    /// accounts.
    #[allow(clippy::new_without_default)]
    #[must_use]
    pub fn new() -> Self {
        Self {
            clients: BTreeMap::new(),
        }
    }

    /// Attempts to apply the specified transaction.
    ///
    /// If the client account referenced by the transaction does not exist,
    /// it will be created.
    ///
    /// # Errors
    ///
    /// If a transaction fails to be applied, an error will be returned. Since
    /// transactions are applied atomically, no changes will be made to the
    /// client account if an error occurs.
    pub fn transact(&mut self, transaction: &Transaction) -> Result<(), TransactionError> {
        let client = match self.clients.entry(transaction.client) {
            btree_map::Entry::Vacant(entry) => entry.insert(ClientAccount::new()),
            btree_map::Entry::Occupied(entry) => entry.into_mut(),
        };

        match transaction.transaction_type {
            TransactionType::Deposit { amount } => client.deposit(transaction.tx, amount),
            TransactionType::Withdrawal { amount } => client.withdraw(transaction.tx, amount),
            TransactionType::Dispute => client.create_dispute(transaction.tx),
            TransactionType::Resolve => {
                client.resolve_dispute(transaction.tx, DisputeResolution::Resolve)
            }
            TransactionType::Chargeback => {
                client.resolve_dispute(transaction.tx, DisputeResolution::Chargeback)
            }
        }
    }

    /// Generates a report containing details of the state of all client
    /// accounts.
    ///
    /// In the case that the client account total funds cause an overflow,
    /// that client will be excluded from the report and an error will
    /// be logged.
    pub fn generate_report(&self) -> impl Iterator<Item = ReportEntry> + '_ {
        self.clients
            .iter()
            .filter_map(|(client_id, client_account)| match client_account.total() {
                Ok(total) => Some(ReportEntry {
                    client: *client_id,
                    available: client_account.available,
                    held: client_account.held,
                    total,
                    locked: client_account.locked,
                }),
                Err(err) => {
                    log::error!(
                        "Skipping account {} due to error finding total: {}",
                        client_id,
                        err
                    );
                    None
                }
            })
    }

    /// Convenience method to convert the report generated by
    /// [`TransactionProcessor::generate_report`] into a `Vec`. Useful
    /// for testing purposes.
    #[must_use]
    pub fn generate_report_as_vec(&self) -> Vec<ReportEntry> {
        self.generate_report().collect()
    }
}

/// A client identifier.
pub type ClientId = u16;
/// A transaction identifier.
pub type TransactionId = u32;

/// A struct representing a transaction.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Transaction {
    /// The client account which the transaction applies to.
    client: ClientId,
    /// The transaction ID. In the case of deposits and withdrawals, this
    /// should be a new transaction ID. In the case of disputes, resolutions,
    /// and chargebacks, this should be the ID of an existing transaction.
    tx: TransactionId,
    /// The type of the transaction, and associated data where relevant.
    transaction_type: TransactionType,
}

impl Transaction {
    /// Creates a new [Transaction] instance.
    ///
    /// * `client` - The client account which the transaction applies to.
    /// * `tx` - The transaction ID. In the case of deposits and withdrawals,
    ///   this should be a new transaction ID. In the case of disputes,
    ///   resolutions, and chargebacks, this should be the ID of an existing
    ///   transaction.
    /// * `transaction_type` - The type of the transaction, and associated data
    ///   where relevant.
    #[must_use]
    pub const fn new(
        client: ClientId,
        tx: TransactionId,
        transaction_type: TransactionType,
    ) -> Self {
        Self {
            client,
            tx,
            transaction_type,
        }
    }
}

/// The type of a transaction, and associated data where relevant.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TransactionType {
    /// Increases the available funds by the specified amount.
    Deposit {
        /// The amount by which to increase the available funds.
        amount: CurrencyAmount,
    },
    /// Reduces the available funds by the specified amount.
    Withdrawal {
        /// The amount by which to decrease the avaiable funds.
        amount: CurrencyAmount,
    },
    /// Disputes the specified transaction in the user's account. All changes
    /// occur atomically.
    ///
    /// This will transfer the value of the transaction from the available
    /// funds to the held funds, and mark the transaction as disputed.
    Dispute,
    /// Resolves a dispute, moving the previously held funds back into
    /// the available balance.
    Resolve,
    /// Performs a chargeback, removing the funds from the held balance,
    /// and locking the account.
    ///
    /// After a chargeback is performed, the transaction cannot be disputed
    /// again.
    Chargeback,
}

#[cfg(test)]
mod test {
    use std::str::FromStr;

    use crate::{
        CurrencyAmount, ReportEntry, Transaction, TransactionError, TransactionProcessor,
        TransactionType,
    };

    #[test]
    fn test_deposit_withdraw() {
        let mut tp = TransactionProcessor::new();

        assert_eq!(tp.generate_report().count(), 0);

        // Try withdrawing some money, should fail
        assert_eq!(
            Err(TransactionError::NotEnoughFunds),
            tp.transact(&Transaction::new(
                1,
                2,
                TransactionType::Withdrawal {
                    amount: CurrencyAmount::from_str("100").unwrap()
                }
            ))
        );

        // Account should exist, but contain no money
        assert_eq!(
            vec![ReportEntry {
                client: 1,
                available: CurrencyAmount::ZERO,
                held: CurrencyAmount::ZERO,
                total: CurrencyAmount::ZERO,
                locked: false
            }],
            tp.generate_report_as_vec()
        );

        let fifty = CurrencyAmount::from_str("50").unwrap();

        // Deposit 50
        tp.transact(&Transaction::new(
            1,
            2,
            TransactionType::Deposit { amount: fifty },
        ))
        .unwrap();

        assert_eq!(
            vec![ReportEntry {
                client: 1,
                available: fifty,
                held: CurrencyAmount::ZERO,
                total: fifty,
                locked: false
            }],
            tp.generate_report_as_vec()
        );

        // Try (and fail) to withdraw 100
        assert_eq!(
            Err(TransactionError::NotEnoughFunds),
            tp.transact(&Transaction::new(
                1,
                2,
                TransactionType::Withdrawal {
                    amount: CurrencyAmount::from_str("100").unwrap()
                }
            ))
        );

        // Account is unchanged
        assert_eq!(
            vec![ReportEntry {
                client: 1,
                available: fifty,
                held: CurrencyAmount::ZERO,
                total: fifty,
                locked: false
            }],
            tp.generate_report_as_vec()
        );

        // Withdraw all (50)
        tp.transact(&Transaction::new(
            1,
            3,
            TransactionType::Withdrawal { amount: fifty },
        ))
        .unwrap();

        assert_eq!(
            vec![ReportEntry {
                client: 1,
                available: CurrencyAmount::ZERO,
                held: CurrencyAmount::ZERO,
                total: CurrencyAmount::ZERO,
                locked: false
            }],
            tp.generate_report_as_vec()
        );
    }
}
