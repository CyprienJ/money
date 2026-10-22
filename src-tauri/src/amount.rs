//! Integer arithmetic on amounts: cents never go through floating point.

/// `a × b / c` rounded half up (away from zero), for non-negative `b` and positive `c`.
pub fn mul_div(a: i64, b: i64, c: i64) -> i64 {
    let (a, b, c) = (i128::from(a), i128::from(b), i128::from(c));
    let product = a * b * 2;
    let rounded = if product >= 0 {
        (product + c) / (c * 2)
    } else {
        (product - c) / (c * 2)
    };
    rounded as i64
}
