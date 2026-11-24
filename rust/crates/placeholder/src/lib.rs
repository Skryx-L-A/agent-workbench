//! Placeholder crate for the stage-0 scaffold. It holds no product logic.

/// Adds two counters and reports an overflow as `None`.
#[must_use]
pub fn checked_sum(a: u32, b: u32) -> Option<u32> {
    a.checked_add(b)
}

#[cfg(test)]
mod tests {
    use super::checked_sum;

    #[test]
    fn adds_in_range() {
        assert_eq!(checked_sum(2, 3), Some(5));
    }

    #[test]
    fn reports_overflow() {
        assert_eq!(checked_sum(u32::MAX, 1), None);
    }
}
