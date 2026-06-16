#![forbid(unsafe_code)]
#[cfg(test)]
mod numeric_test;
#[cfg(test)]
mod pseudobigint_test;

#[derive(Clone, Copy, Debug, Default)]
pub struct Number(pub f64);

pub const MAX_SAFE_INTEGER: Number = Number((1_u64 << 53) as f64 - 1.0);
pub const MIN_SAFE_INTEGER: Number = Number(-MAX_SAFE_INTEGER.0);

pub fn compare(a: Number, b: Number) -> i32 {
    if a.0.is_nan() || b.0.is_nan() {
        return match (a.0.is_nan(), b.0.is_nan()) {
            (true, true) => 0,
            (true, false) => 1,
            (false, true) => -1,
            (false, false) => unreachable!(),
        };
    }
    if a.0 < b.0 {
        -1
    } else if a.0 > b.0 {
        1
    } else {
        0
    }
}

impl Number {
    pub fn is_nan(self) -> bool {
        self.0.is_nan()
    }

    pub fn is_inf(self) -> bool {
        self.0.is_infinite()
    }

    fn is_non_finite(value: f64) -> bool {
        // This is equivalent to checking `math.IsNaN(x) || math.IsInf(x, 0)` in one operation.
        const MASK: u64 = 0x7FF0000000000000;
        value.to_bits() & MASK == MASK
    }

    // https://tc39.es/ecma262/2024/multipage/abstract-operations.html#sec-touint32
    fn to_uint32(self) -> u32 {
        // The only difference between ToUint32 and ToInt32 is the interpretation of the bits.
        self.to_int32() as u32
    }

    // https://tc39.es/ecma262/2024/multipage/abstract-operations.html#sec-toint32
    fn to_int32(self) -> i32 {
        let mut x = self.0;

        // Fast path: if the number is in the range (-2^31, 2^32), i.e. an SMI,
        // then we don't need to do any special mapping.
        let smi = x as i32;
        if smi as f64 == x {
            return smi;
        }

        // 2. If number is not finite or number is either +0F or -0F, return +0F.
        // Zero was covered by the test above.
        if Self::is_non_finite(x) {
            return 0;
        }

        // Let int be truncate(R(number)).
        x = x.trunc();
        // Let int32bit be int modulo 2**32.
        x %= 4294967296.0;
        // If int32bit >= 2**31, return F(int32bit - 2**32); otherwise return F(int32bit).
        x as i64 as i32
    }

    fn to_shift_count(self) -> u32 {
        self.to_uint32() & 31
    }

    pub fn bitwise_not(self) -> Self {
        Self((!self.to_int32()) as f64)
    }

    pub fn bitwise_or(self, rhs: Self) -> Self {
        Self((self.to_int32() | rhs.to_int32()) as f64)
    }

    pub fn bitwise_and(self, rhs: Self) -> Self {
        Self((self.to_int32() & rhs.to_int32()) as f64)
    }

    pub fn bitwise_xor(self, rhs: Self) -> Self {
        Self((self.to_int32() ^ rhs.to_int32()) as f64)
    }

    // https://tc39.es/ecma262/2024/multipage/ecmascript-data-types-and-values.html#sec-numeric-types-number-signedRightShift
    pub fn signed_right_shift(self, rhs: Self) -> Self {
        Self((self.to_int32() >> rhs.to_shift_count()) as f64)
    }

    // https://tc39.es/ecma262/2024/multipage/ecmascript-data-types-and-values.html#sec-numeric-types-number-unsignedRightShift
    pub fn unsigned_right_shift(self, rhs: Self) -> Self {
        Self((self.to_uint32() >> rhs.to_shift_count()) as f64)
    }

    // https://tc39.es/ecma262/2024/multipage/ecmascript-data-types-and-values.html#sec-numeric-types-number-leftShift
    pub fn left_shift(self, rhs: Self) -> Self {
        Self((self.to_int32() << rhs.to_shift_count()) as f64)
    }

    pub fn remainder(self, rhs: Self) -> Self {
        Self(self.0 % rhs.0)
    }

    pub fn exponentiate(self, rhs: Self) -> Self {
        Self(self.0.powf(rhs.0))
    }
}

impl From<i32> for Number {
    fn from(value: i32) -> Self {
        Self(value as f64)
    }
}

impl std::str::FromStr for Number {
    type Err = std::num::ParseFloatError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        s.parse::<f64>().map(Self)
    }
}

pub fn from_string(text: &str) -> Number {
    if let Some(number) = try_parse_prefixed_integer(text) {
        return number;
    }
    text.parse().unwrap_or(Number(f64::NAN))
}

fn try_parse_prefixed_integer(text: &str) -> Option<Number> {
    let text = text.trim();
    let (digits, radix) = text
        .strip_prefix("0x")
        .or_else(|| text.strip_prefix("0X"))
        .map(|digits| (digits, 16))
        .or_else(|| {
            text.strip_prefix("0b")
                .or_else(|| text.strip_prefix("0B"))
                .map(|digits| (digits, 2))
        })
        .or_else(|| {
            text.strip_prefix("0o")
                .or_else(|| text.strip_prefix("0O"))
                .map(|digits| (digits, 8))
        })?;

    let mut value = 0.0;
    for ch in digits.chars() {
        if ch == '_' {
            continue;
        }
        let digit = ch.to_digit(radix)? as f64;
        value = value * radix as f64 + digit;
    }

    Some(Number(value))
}

impl std::fmt::Display for Number {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.0.is_nan() {
            f.write_str("NaN")
        } else if self.0.is_infinite() {
            if self.0.is_sign_negative() {
                f.write_str("-Infinity")
            } else {
                f.write_str("Infinity")
            }
        } else if self.0 == 0.0 {
            f.write_str("0")
        } else if self.0.abs() >= 1e21 || self.0.abs() < 1e-6 {
            f.write_str(&format_js_exponential(self.0))
        } else if self.0.fract() == 0.0 {
            write!(f, "{:.0}", self.0)
        } else {
            write!(f, "{}", self.0)
        }
    }
}

fn format_js_exponential(value: f64) -> String {
    let raw = format!("{value:e}");
    let Some((mantissa, exponent)) = raw.split_once('e') else {
        return raw;
    };
    let mantissa = mantissa
        .trim_end_matches('0')
        .trim_end_matches('.')
        .to_string();
    let exponent_value = exponent.parse::<i32>().unwrap_or(0);
    if exponent_value >= 0 {
        format!("{mantissa}e+{exponent_value}")
    } else {
        format!("{mantissa}e{exponent_value}")
    }
}

impl std::ops::Neg for Number {
    type Output = Self;

    fn neg(self) -> Self::Output {
        Self(-self.0)
    }
}

macro_rules! number_binop {
    ($trait:ident, $method:ident, $op:tt) => {
        impl std::ops::$trait for Number {
            type Output = Self;

            fn $method(self, rhs: Self) -> Self::Output {
                Self(self.0 $op rhs.0)
            }
        }
    };
}

number_binop!(Add, add, +);
number_binop!(Sub, sub, -);
number_binop!(Mul, mul, *);
number_binop!(Div, div, /);

impl PartialEq for Number {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0 || (self.0.is_nan() && other.0.is_nan())
    }
}

impl Eq for Number {}

impl std::hash::Hash for Number {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        let bits = if self.0 == 0.0 {
            0.0f64.to_bits()
        } else if self.0.is_nan() {
            f64::NAN.to_bits()
        } else {
            self.0.to_bits()
        };
        bits.hash(state);
    }
}

// PseudoBigInt represents a JS-like bigint. The zero state of the struct represents the value 0.
#[derive(Debug, Clone, Eq, Hash, PartialEq, Default)]
pub struct PseudoBigInt {
    pub negative: bool,       // true if the value is a non-zero negative number.
    pub base10_value: String, // The absolute value in base 10 with no leading zeros. The value zero is represented as an empty string.
}

pub fn new_pseudo_big_int(value: &str, negative: bool) -> PseudoBigInt {
    let value = value.trim_start_matches('0').to_owned();
    PseudoBigInt {
        negative: negative && !value.is_empty(),
        base10_value: value,
    }
}

impl std::fmt::Display for PseudoBigInt {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.base10_value.is_empty() {
            return f.write_str("0");
        }
        if self.negative {
            return write!(f, "-{}", self.base10_value);
        }
        f.write_str(&self.base10_value)
    }
}

impl PseudoBigInt {
    pub fn sign(&self) -> i32 {
        if self.base10_value.is_empty() {
            return 0;
        }
        if self.negative {
            return -1;
        }
        1
    }
}

pub fn parse_valid_big_int(text: &str) -> PseudoBigInt {
    let (text, negative) = text
        .strip_prefix('-')
        .map(|text| (text, true))
        .unwrap_or((text, false));
    new_pseudo_big_int(&parse_pseudo_big_int(text), negative)
}

pub fn parse_pseudo_big_int(string_value: &str) -> String {
    let string_value = string_value.strip_suffix('n').unwrap_or(string_value);
    let b1 = string_value.as_bytes().get(1).copied().unwrap_or_default();
    match b1 {
        b'b' | b'B' | b'o' | b'O' | b'x' | b'X' => {
            // Not decimal.
        }
        _ => {
            let string_value = string_value.trim_start_matches('0');
            if string_value.is_empty() {
                return "0".to_owned();
            }
            return string_value.to_owned();
        }
    }

    parse_prefixed_integer_to_decimal(string_value)
        .unwrap_or_else(|| panic!("Failed to parse big int: {string_value:?}"))
}

fn parse_prefixed_integer_to_decimal(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    if bytes.len() < 2 || bytes[0] != b'0' {
        return None;
    }
    let base = match bytes[1] {
        b'b' | b'B' => 2,
        b'o' | b'O' => 8,
        b'x' | b'X' => 16,
        _ => return None,
    };

    let mut decimal = "0".to_owned();
    for ch in text[2..].chars() {
        if ch == '_' {
            continue;
        }
        let digit = ch.to_digit(base)?;
        decimal_mul_small_add(&mut decimal, base, digit);
    }
    Some(decimal)
}

fn decimal_mul_small_add(decimal: &mut String, mul: u32, add: u32) {
    let mut carry = add;
    let mut out = Vec::with_capacity(decimal.len() + 2);
    for byte in decimal.as_bytes().iter().rev() {
        let value = ((*byte - b'0') as u32) * mul + carry;
        out.push((value % 10) as u8 + b'0');
        carry = value / 10;
    }
    while carry > 0 {
        out.push((carry % 10) as u8 + b'0');
        carry /= 10;
    }
    out.reverse();
    *decimal = String::from_utf8(out).unwrap_or_else(|err| panic!("{err}"));
    let trimmed = decimal.trim_start_matches('0');
    if trimmed.is_empty() {
        decimal.clear();
        decimal.push('0');
    } else if trimmed.len() != decimal.len() {
        *decimal = trimmed.to_owned();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn number_zero_key_matches_js_map_semantics() {
        let positive_zero = Number(0.0);
        let negative_zero = Number(-0.0);

        assert_eq!(positive_zero, negative_zero);
        assert_eq!(compare(negative_zero, positive_zero), 0);

        let mut map = HashMap::new();
        map.insert(positive_zero, "zero");
        assert_eq!(map.get(&negative_zero), Some(&"zero"));
    }
}
