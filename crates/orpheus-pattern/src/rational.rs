use core::cmp::Ordering;
use core::ops::Add;

use crate::PatternError;

/// Exact rational time value stored in normalized form.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct Rational {
    numerator: i128,
    denominator: i128,
}

impl Rational {
    /// Creates a normalized rational with a positive denominator.
    ///
    /// # Errors
    ///
    /// Returns [`PatternError::InvalidDenominator`] if `denominator` is zero.
    pub fn new(numerator: i64, denominator: i64) -> Result<Self, PatternError> {
        if denominator == 0 {
            return Err(PatternError::InvalidDenominator { denominator });
        }

        Ok(Self::normalize(
            i128::from(numerator),
            i128::from(denominator),
        ))
    }

    /// Returns the additive identity.
    #[must_use]
    pub const fn zero() -> Self {
        Self {
            numerator: 0,
            denominator: 1,
        }
    }

    /// Returns the multiplicative identity.
    #[must_use]
    pub const fn one() -> Self {
        Self {
            numerator: 1,
            denominator: 1,
        }
    }

    /// Returns the normalized numerator.
    #[must_use]
    pub const fn numerator(&self) -> i128 {
        self.numerator
    }

    /// Returns the normalized denominator.
    #[must_use]
    pub const fn denominator(&self) -> i128 {
        self.denominator
    }

    /// Adds two rationals using checked intermediate arithmetic.
    ///
    /// # Errors
    ///
    /// Returns [`PatternError::ArithmeticOverflow`] if the intermediate
    /// numerator or denominator exceeds the supported integer range.
    pub fn checked_add(&self, rhs: &Self) -> Result<Self, PatternError> {
        let common_divisor = gcd(self.denominator, rhs.denominator);
        let left_scale = rhs.denominator / common_divisor;
        let right_scale = self.denominator / common_divisor;

        let left_numerator =
            self.numerator
                .checked_mul(left_scale)
                .ok_or(PatternError::ArithmeticOverflow {
                    operation: "rational addition",
                })?;
        let right_numerator =
            rhs.numerator
                .checked_mul(right_scale)
                .ok_or(PatternError::ArithmeticOverflow {
                    operation: "rational addition",
                })?;
        let numerator = left_numerator.checked_add(right_numerator).ok_or(
            PatternError::ArithmeticOverflow {
                operation: "rational addition",
            },
        )?;
        let denominator =
            self.denominator
                .checked_mul(left_scale)
                .ok_or(PatternError::ArithmeticOverflow {
                    operation: "rational addition",
                })?;

        Ok(Self::normalize(numerator, denominator))
    }

    /// Compares two rationals using checked intermediate arithmetic.
    ///
    /// # Errors
    ///
    /// Returns [`PatternError::ArithmeticOverflow`] if the comparison
    /// requires an intermediate value outside the supported range.
    pub fn checked_cmp(&self, other: &Self) -> Result<Ordering, PatternError> {
        let common_divisor = gcd(self.denominator, other.denominator);
        let left_scale = other.denominator / common_divisor;
        let right_scale = self.denominator / common_divisor;
        let left =
            self.numerator
                .checked_mul(left_scale)
                .ok_or(PatternError::ArithmeticOverflow {
                    operation: "rational comparison",
                })?;
        let right =
            other
                .numerator
                .checked_mul(right_scale)
                .ok_or(PatternError::ArithmeticOverflow {
                    operation: "rational comparison",
                })?;

        Ok(left.cmp(&right))
    }

    const fn normalize(numerator: i128, denominator: i128) -> Self {
        if numerator == 0 {
            return Self::zero();
        }

        let (normalized_numerator, normalized_denominator) = if denominator < 0 {
            (-numerator, -denominator)
        } else {
            (numerator, denominator)
        };
        let divisor = gcd(normalized_numerator.abs(), normalized_denominator);

        Self {
            numerator: normalized_numerator / divisor,
            denominator: normalized_denominator / divisor,
        }
    }
}

impl Add for Rational {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        match self.checked_add(&rhs) {
            Ok(sum) => sum,
            Err(PatternError::ArithmeticOverflow { .. }) => {
                panic!("rational addition overflowed during checked arithmetic")
            }
            Err(PatternError::InvalidDenominator { .. } | PatternError::InvalidSpan { .. }) => {
                unreachable!("checked_add only reports arithmetic overflow")
            }
        }
    }
}

impl Ord for Rational {
    fn cmp(&self, other: &Self) -> Ordering {
        match self.checked_cmp(other) {
            Ok(ordering) => ordering,
            Err(PatternError::ArithmeticOverflow { .. }) => {
                panic!("rational comparison overflowed during checked arithmetic")
            }
            Err(PatternError::InvalidDenominator { .. } | PatternError::InvalidSpan { .. }) => {
                unreachable!("checked_cmp only reports arithmetic overflow")
            }
        }
    }
}

impl PartialOrd for Rational {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

const fn gcd(mut left: i128, mut right: i128) -> i128 {
    while right != 0 {
        let remainder = left % right;
        left = right;
        right = remainder;
    }

    left.abs()
}
