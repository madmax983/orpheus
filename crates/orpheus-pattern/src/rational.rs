//! The `rational` module implements exact rational numbers for continuous time.
//!
//! Orpheus uses rational numbers to avoid floating-point drift when sequencing
//! repeating patterns.

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
    /// The numerator and denominator are reduced by their greatest common
    /// divisor. Negative signs are always pulled into the numerator.
    ///
    /// # Examples
    ///
    /// ```
    /// use orpheus_pattern::Rational;
    ///
    /// // Fractions are reduced to their simplest form.
    /// let two_fourths = Rational::new(2, 4).unwrap();
    /// assert_eq!(two_fourths.numerator(), 1);
    /// assert_eq!(two_fourths.denominator(), 2);
    ///
    /// // Denominators are always positive.
    /// let negative_half = Rational::new(1, -2).unwrap();
    /// assert_eq!(negative_half.numerator(), -1);
    /// assert_eq!(negative_half.denominator(), 2);
    /// ```
    ///
    /// # Errors
    ///
    /// Returns [`PatternError::InvalidDenominator`] if `denominator` is zero.
    pub fn new(numerator: i64, denominator: i64) -> Result<Self, PatternError> {
        if denominator == 0 {
            return Err(PatternError::InvalidDenominator { denominator });
        }

        Self::checked_normalize(i128::from(numerator), i128::from(denominator))
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
    ///
    /// Use this instead of the `Add` impl if unrepresentable sums must be
    /// reported as data instead of panicking.
    ///
    /// ## Examples
    ///
    /// ```
    /// use orpheus_pattern::Rational;
    ///
    /// let one_half = Rational::new(1, 2).unwrap();
    /// let one_quarter = Rational::new(1, 4).unwrap();
    /// let sum = one_half.checked_add(&one_quarter).unwrap();
    /// assert_eq!(sum.numerator(), 3);
    /// assert_eq!(sum.denominator(), 4);
    /// ```
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

        Self::checked_normalize(numerator, denominator)
    }

    /// Subtracts two rationals using checked intermediate arithmetic.
    ///
    /// # Errors
    ///
    /// Returns [`PatternError::ArithmeticOverflow`] if the intermediate
    /// numerator or denominator exceeds the supported integer range.
    ///
    /// ## Examples
    ///
    /// ```
    /// use orpheus_pattern::Rational;
    ///
    /// let one_half = Rational::new(1, 2).unwrap();
    /// let one_quarter = Rational::new(1, 4).unwrap();
    /// let diff = one_half.checked_sub(&one_quarter).unwrap();
    /// assert_eq!(diff.numerator(), 1);
    /// assert_eq!(diff.denominator(), 4);
    /// ```
    pub fn checked_sub(&self, rhs: &Self) -> Result<Self, PatternError> {
        let negated_rhs = Self::checked_normalize(rhs.numerator, -rhs.denominator)?;
        self.checked_add(&negated_rhs)
    }

    /// Multiplies two rationals using checked intermediate arithmetic.
    ///
    /// # Errors
    ///
    /// Returns [`PatternError::ArithmeticOverflow`] if the intermediate
    /// numerator or denominator exceeds the supported integer range.
    ///
    /// ## Examples
    ///
    /// ```
    /// use orpheus_pattern::Rational;
    ///
    /// let one_half = Rational::new(1, 2).unwrap();
    /// let three_quarters = Rational::new(3, 4).unwrap();
    /// let product = one_half.checked_mul(&three_quarters).unwrap();
    /// assert_eq!(product.numerator(), 3);
    /// assert_eq!(product.denominator(), 8);
    /// ```
    pub fn checked_mul(&self, rhs: &Self) -> Result<Self, PatternError> {
        let numerator =
            self.numerator
                .checked_mul(rhs.numerator)
                .ok_or(PatternError::ArithmeticOverflow {
                    operation: "rational multiplication",
                })?;
        let denominator = self.denominator.checked_mul(rhs.denominator).ok_or(
            PatternError::ArithmeticOverflow {
                operation: "rational multiplication",
            },
        )?;

        Self::checked_normalize(numerator, denominator)
    }

    /// Compares two rationals using an overflow-free continued-fraction walk.
    ///
    /// # Errors
    ///
    /// This comparison algorithm is overflow-free for valid rationals and
    /// currently returns `Ok` for all values constructible through this crate.
    ///
    /// ## Examples
    ///
    /// ```
    /// use orpheus_pattern::Rational;
    /// use core::cmp::Ordering;
    ///
    /// let one_half = Rational::new(1, 2).unwrap();
    /// let two_quarters = Rational::new(2, 4).unwrap();
    /// let three_quarters = Rational::new(3, 4).unwrap();
    ///
    /// assert_eq!(one_half.checked_cmp(&two_quarters).unwrap(), Ordering::Equal);
    /// assert_eq!(one_half.checked_cmp(&three_quarters).unwrap(), Ordering::Less);
    /// ```
    pub fn checked_cmp(&self, other: &Self) -> Result<Ordering, PatternError> {
        Ok(compare_rationals(self, other))
    }

    /// Creates a normalized rational directly from `i128` parts.
    ///
    /// # Errors
    ///
    /// Returns [`PatternError::InvalidDenominator`] if `denominator` is zero,
    /// or [`PatternError::ArithmeticOverflow`] if normalization cannot be
    /// represented in the bounded runtime domain.
    ///
    /// ## Examples
    ///
    /// ```
    /// use orpheus_pattern::Rational;
    ///
    /// let r = Rational::checked_from_parts(2, 4).unwrap();
    /// assert_eq!(r.numerator(), 1);
    /// assert_eq!(r.denominator(), 2);
    /// ```
    pub fn checked_from_parts(numerator: i128, denominator: i128) -> Result<Self, PatternError> {
        Self::checked_normalize(numerator, denominator)
    }

    fn checked_normalize(numerator: i128, denominator: i128) -> Result<Self, PatternError> {
        if denominator == 0 {
            return Err(PatternError::InvalidDenominator { denominator: 0 });
        }

        if numerator == 0 {
            return Ok(Self::zero());
        }

        let operation = "rational normalization";
        let numerator_negative = numerator < 0;
        let denominator_negative = denominator < 0;
        let numerator_magnitude = numerator.unsigned_abs();
        let denominator_magnitude = denominator.unsigned_abs();
        let divisor = gcd_u128(numerator_magnitude, denominator_magnitude);
        let reduced_numerator = numerator_magnitude / divisor;
        let reduced_denominator = denominator_magnitude / divisor;
        let normalized_numerator = signed_from_magnitude(
            reduced_numerator,
            numerator_negative ^ denominator_negative,
            operation,
        )?;
        let normalized_denominator = positive_from_magnitude(reduced_denominator, operation)?;

        Ok(Self {
            numerator: normalized_numerator,
            denominator: normalized_denominator,
        })
    }
}

/// Adds two rationals.
///
/// # Panics
///
/// Panics if the exact sum cannot be represented in the bounded `i128`
/// runtime representation. Use [`Rational::checked_add`] to handle that case
/// explicitly.
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
        compare_rationals(self, other)
    }
}

impl PartialOrd for Rational {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

fn gcd(left: i128, right: i128) -> i128 {
    let divisor = gcd_u128(left.unsigned_abs(), right.unsigned_abs());
    // Stored denominators are always positive i128 values, so their gcd fits too.
    i128::try_from(divisor)
        .unwrap_or_else(|_| unreachable!("gcd of stored denominators always fits in i128"))
}

const fn gcd_u128(mut left: u128, mut right: u128) -> u128 {
    while right != 0 {
        let remainder = left % right;
        left = right;
        right = remainder;
    }

    left
}

fn positive_from_magnitude(magnitude: u128, operation: &'static str) -> Result<i128, PatternError> {
    i128::try_from(magnitude).map_err(|_| PatternError::ArithmeticOverflow { operation })
}

fn signed_from_magnitude(
    magnitude: u128,
    negative: bool,
    operation: &'static str,
) -> Result<i128, PatternError> {
    const I128_MIN_MAGNITUDE: u128 = 1_u128 << 127;

    if negative {
        if magnitude == I128_MIN_MAGNITUDE {
            return Ok(i128::MIN);
        }

        let value = i128::try_from(magnitude)
            .map_err(|_| PatternError::ArithmeticOverflow { operation })?;
        Ok(-value)
    } else {
        i128::try_from(magnitude).map_err(|_| PatternError::ArithmeticOverflow { operation })
    }
}

fn compare_rationals(left: &Rational, right: &Rational) -> Ordering {
    if left.numerator.signum() != right.numerator.signum() {
        return left.numerator.cmp(&right.numerator);
    }

    let ordering = compare_positive_rationals(
        left.numerator.unsigned_abs(),
        left.denominator.unsigned_abs(),
        right.numerator.unsigned_abs(),
        right.denominator.unsigned_abs(),
    );

    if left.numerator < 0 {
        ordering.reverse()
    } else {
        ordering
    }
}

fn compare_positive_rationals(
    mut left_numerator: u128,
    mut left_denominator: u128,
    mut right_numerator: u128,
    mut right_denominator: u128,
) -> Ordering {
    let mut reversed = false;

    loop {
        let left_integer = left_numerator / left_denominator;
        let right_integer = right_numerator / right_denominator;
        if left_integer != right_integer {
            let ordering = left_integer.cmp(&right_integer);
            return if reversed {
                ordering.reverse()
            } else {
                ordering
            };
        }

        let left_remainder = left_numerator % left_denominator;
        let right_remainder = right_numerator % right_denominator;
        let ordering = match (left_remainder == 0, right_remainder == 0) {
            (true, true) => Ordering::Equal,
            (true, false) => Ordering::Less,
            (false, true) => Ordering::Greater,
            (false, false) => {
                left_numerator = left_denominator;
                left_denominator = left_remainder;
                right_numerator = right_denominator;
                right_denominator = right_remainder;
                reversed = !reversed;
                continue;
            }
        };

        return if reversed {
            ordering.reverse()
        } else {
            ordering
        };
    }
}

impl From<&Rational> for f64 {
    fn from(value: &Rational) -> Self {
        #[allow(clippy::cast_precision_loss)]
        let num = value.numerator as Self;
        #[allow(clippy::cast_precision_loss)]
        let den = value.denominator as Self;
        num / den
    }
}

impl From<Rational> for f64 {
    fn from(value: Rational) -> Self {
        Self::from(&value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checked_normalize_accepts_i128_min_numerator() {
        let rational = Rational::checked_normalize(i128::MIN, 1).unwrap();

        assert_eq!(rational.numerator(), i128::MIN);
        assert_eq!(rational.denominator(), 1);
    }

    #[test]
    fn checked_cmp_handles_large_cross_products_without_overflow() {
        let larger = Rational::checked_normalize(i128::MAX - 1, i128::MAX).unwrap();
        let smaller = Rational::checked_normalize(i128::MAX - 2, i128::MAX - 1).unwrap();

        assert_eq!(larger.checked_cmp(&smaller), Ok(Ordering::Greater));
        assert_eq!(larger.cmp(&smaller), Ordering::Greater);
    }

    #[test]
    fn checked_normalize_rejects_zero_denominator() {
        let result = Rational::checked_from_parts(1, 0);
        assert_eq!(
            result,
            Err(PatternError::InvalidDenominator { denominator: 0 })
        );
    }

    #[test]
    #[allow(clippy::float_cmp)]
    fn rational_converts_to_f64() {
        let r = Rational::new(3, 4).unwrap();
        let f: f64 = r.into();
        assert_eq!(f, 0.75);

        let ref_r = &Rational::new(1, 2).unwrap();
        let f2: f64 = ref_r.into();
        assert_eq!(f2, 0.5);
    }
}
