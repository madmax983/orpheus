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
        Self::normalize(
            (self.numerator * rhs.denominator) + (rhs.numerator * self.denominator),
            self.denominator * rhs.denominator,
        )
    }
}

impl Ord for Rational {
    fn cmp(&self, other: &Self) -> Ordering {
        (self.numerator * other.denominator).cmp(&(other.numerator * self.denominator))
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
