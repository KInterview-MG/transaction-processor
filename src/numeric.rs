use std::fmt::{Display, Formatter};
use std::ops::{Add, Neg, Sub};
use std::str::FromStr;

use rust_decimal::Decimal;
use serde::de::Error;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// An error occurred when performing arithmetic operations on
/// currency amounts.
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum CurrencyError {
    /// The result of the calculation would overflow/underflow.
    OutOfBounds,
}

impl Display for CurrencyError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            CurrencyError::OutOfBounds => "Out of bounds",
        })
    }
}

/// Error occurring when parsing a string to a currency amount.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CurrencyAmountParseError {
    /// The specified string is not a valid currency amount.
    InvalidNumericValue,
}

impl Display for CurrencyAmountParseError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            CurrencyAmountParseError::InvalidNumericValue => "Invalid numeric value",
        })
    }
}

/// An amount of money, represented as a decimal number.
///
/// For `x` decimal places of precision, this can handle positive and negative
/// values with magnitude `(2^96)/(10^x)`. For four decimal places, this is
/// approximately `2^82`.
///
/// All arithmetic operations are checked and return a result type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct CurrencyAmount {
    value: Decimal,
}

impl CurrencyAmount {
    /// Constant value of `0.0`.
    pub const ZERO: Self = Self {
        value: Decimal::ZERO,
    };

    /// Returns true if this value is less than zero.
    #[must_use]
    pub fn is_negative(&self) -> bool {
        self.value.lt(&Decimal::ZERO)
    }
}

impl Add for CurrencyAmount {
    type Output = Result<Self, CurrencyError>;

    fn add(self, rhs: Self) -> Self::Output {
        // Always ensure that the addition is safe
        Ok(Self {
            value: self
                .value
                .checked_add(rhs.value)
                .ok_or(CurrencyError::OutOfBounds)?,
        })
    }
}

impl Sub for CurrencyAmount {
    type Output = Result<Self, CurrencyError>;

    fn sub(self, rhs: Self) -> Result<Self, CurrencyError> {
        // Always ensure that the subtraction is safe
        Ok(Self {
            value: self
                .value
                .checked_sub(rhs.value)
                .ok_or(CurrencyError::OutOfBounds)?,
        })
    }
}

impl Neg for CurrencyAmount {
    type Output = Self;

    fn neg(self) -> Self::Output {
        Self {
            value: self.value.neg(),
        }
    }
}

impl FromStr for CurrencyAmount {
    type Err = CurrencyAmountParseError;

    fn from_str(value: &str) -> Result<Self, CurrencyAmountParseError> {
        Ok(Self {
            value: Decimal::from_str(value)
                .map_err(|_| CurrencyAmountParseError::InvalidNumericValue)?,
        })
    }
}

impl Display for CurrencyAmount {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.value.to_string())
    }
}

impl Serialize for CurrencyAmount {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for CurrencyAmount {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::from_str(&value).map_err(D::Error::custom)
    }
}

#[cfg(test)]
mod test {
    use std::ops::Sub;
    use std::str::FromStr;

    use crate::numeric::CurrencyAmountParseError;
    use crate::CurrencyAmount;

    #[test]
    fn test_parse() {
        assert_eq!("12", CurrencyAmount::from_str("12").unwrap().to_string());
        assert_eq!("12", CurrencyAmount::from_str("12.").unwrap().to_string());
        assert_eq!(
            "12.0",
            CurrencyAmount::from_str("12.0").unwrap().to_string()
        );
        assert_eq!(
            "12.3",
            CurrencyAmount::from_str("12.3").unwrap().to_string()
        );
        assert_eq!(
            "12.34",
            CurrencyAmount::from_str("12.34").unwrap().to_string()
        );
        assert_eq!(
            "12.345",
            CurrencyAmount::from_str("12.345").unwrap().to_string()
        );
        assert_eq!(
            "12.3456",
            CurrencyAmount::from_str("12.3456").unwrap().to_string()
        );
        assert_eq!(
            "12.34567",
            CurrencyAmount::from_str("12.34567").unwrap().to_string()
        );
        assert_eq!(
            "0.34567",
            CurrencyAmount::from_str("00.34567").unwrap().to_string()
        );
        assert_eq!(
            "0.34567",
            CurrencyAmount::from_str("0.34567").unwrap().to_string()
        );
        assert_eq!(
            "0.34567",
            CurrencyAmount::from_str(".34567").unwrap().to_string()
        );
        assert_eq!(
            "0.3456",
            CurrencyAmount::from_str(".3456").unwrap().to_string()
        );
        assert_eq!(
            "0.345",
            CurrencyAmount::from_str(".345").unwrap().to_string()
        );
        assert_eq!("0.0", CurrencyAmount::from_str("0.0").unwrap().to_string());
        assert_eq!("0", CurrencyAmount::from_str("0.").unwrap().to_string());
        assert_eq!("0.0", CurrencyAmount::from_str(".0").unwrap().to_string());

        assert_eq!(
            format!("{}", i64::MAX),
            CurrencyAmount::from_str(&i64::MAX.to_string())
                .unwrap()
                .to_string()
        );
    }

    #[test]
    fn test_parse_negative() {
        assert_eq!("-12", CurrencyAmount::from_str("-12").unwrap().to_string());
        assert_eq!("-12", CurrencyAmount::from_str("-12.").unwrap().to_string());
        assert_eq!(
            "-12.0",
            CurrencyAmount::from_str("-12.0").unwrap().to_string()
        );
        assert_eq!(
            "-12.3",
            CurrencyAmount::from_str("-12.3").unwrap().to_string()
        );
        assert_eq!(
            "-12.34",
            CurrencyAmount::from_str("-12.34").unwrap().to_string()
        );
        assert_eq!(
            "-12.345",
            CurrencyAmount::from_str("-12.345").unwrap().to_string()
        );
        assert_eq!(
            "-12.3456",
            CurrencyAmount::from_str("-12.3456").unwrap().to_string()
        );
        assert_eq!(
            "-12.34567",
            CurrencyAmount::from_str("-12.34567").unwrap().to_string()
        );
        assert_eq!(
            "-0.34567",
            CurrencyAmount::from_str("-00.34567").unwrap().to_string()
        );
        assert_eq!(
            "-0.34567",
            CurrencyAmount::from_str("-0.34567").unwrap().to_string()
        );
        assert_eq!(
            "-0.34567",
            CurrencyAmount::from_str("-.34567").unwrap().to_string()
        );
        assert_eq!(
            "-0.3456",
            CurrencyAmount::from_str("-.3456").unwrap().to_string()
        );
        assert_eq!(
            "-0.345",
            CurrencyAmount::from_str("-.345").unwrap().to_string()
        );
        assert_eq!("0", CurrencyAmount::from_str("-0").unwrap().to_string());
        assert_eq!("0", CurrencyAmount::from_str("-0.").unwrap().to_string());
        assert_eq!("0.0", CurrencyAmount::from_str("-.0").unwrap().to_string());

        assert_eq!(
            format!("{}", i64::MIN),
            CurrencyAmount::from_str(&i64::MIN.to_string())
                .unwrap()
                .to_string()
        );
    }

    #[test]
    fn test_parse_fail() {
        assert_eq!(
            Err(CurrencyAmountParseError::InvalidNumericValue),
            CurrencyAmount::from_str("a")
        );
        assert_eq!(
            Err(CurrencyAmountParseError::InvalidNumericValue),
            CurrencyAmount::from_str("a.0")
        );
        assert_eq!(
            Err(CurrencyAmountParseError::InvalidNumericValue),
            CurrencyAmount::from_str("0.a")
        );
        assert_eq!(
            Err(CurrencyAmountParseError::InvalidNumericValue),
            CurrencyAmount::from_str("..")
        );
        assert_eq!(
            Err(CurrencyAmountParseError::InvalidNumericValue),
            CurrencyAmount::from_str(&i128::MAX.to_string())
        );
        assert_eq!(
            Err(CurrencyAmountParseError::InvalidNumericValue),
            CurrencyAmount::from_str("0.-5")
        );
        assert_eq!(
            Err(CurrencyAmountParseError::InvalidNumericValue),
            CurrencyAmount::from_str(".")
        );
    }

    #[test]
    fn test_add() {
        assert_eq!(
            Ok(CurrencyAmount::from_str("579").unwrap()),
            CurrencyAmount::from_str("123").unwrap() + CurrencyAmount::from_str("456").unwrap()
        );
        assert_eq!(
            Ok(CurrencyAmount::from_str("123.1").unwrap()),
            CurrencyAmount::from_str("123").unwrap() + CurrencyAmount::from_str("0.1").unwrap()
        );
        assert_eq!(
            Ok(CurrencyAmount::from_str(&2_i128.pow(96).sub(2).to_string()).unwrap()),
            CurrencyAmount::from_str(&2_i128.pow(95).sub(1).to_string()).unwrap()
                + CurrencyAmount::from_str(&2_i128.pow(95).sub(1).to_string()).unwrap()
        );
    }

    #[test]
    fn test_sub() {
        assert_eq!(
            Ok(CurrencyAmount::from_str("-333").unwrap()),
            CurrencyAmount::from_str("123").unwrap() - CurrencyAmount::from_str("456").unwrap()
        );
        assert_eq!(
            Ok(CurrencyAmount::from_str("122.9").unwrap()),
            CurrencyAmount::from_str("123").unwrap() - CurrencyAmount::from_str("0.1").unwrap()
        );
        assert_eq!(
            Ok(CurrencyAmount::from_str("0").unwrap()),
            CurrencyAmount::from_str(&2_i128.pow(95).sub(1).to_string()).unwrap()
                - CurrencyAmount::from_str(&2_i128.pow(95).sub(1).to_string()).unwrap()
        );
    }
}
