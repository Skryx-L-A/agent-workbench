//! The value of a secret in memory. It is never printed (`Debug` shows only the length class) and zeroed on drop.

/// The bytes of one secret.
pub struct Geheim(Vec<u8>);

impl Geheim {
    /// Wraps the bytes.
    #[must_use]
    pub fn neu(bytes: Vec<u8>) -> Self {
        Geheim(bytes)
    }

    /// The bytes; only for the call that uses them and for the leak lock.
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.0
    }
}

impl std::fmt::Debug for Geheim {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Geheim(..)")
    }
}

impl Drop for Geheim {
    fn drop(&mut self) {
        self.0.fill(0);
        std::hint::black_box(&self.0);
        // the unit test sees the bytes after the zeroing; without `unsafe` (the workspace forbids it) no test can look
        // at the memory after the drop
        #[cfg(test)]
        tests::nach_dem_nullen(&self.0);
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use super::Geheim;

    thread_local! {
        static GESEHEN: RefCell<Vec<Vec<u8>>> = const { RefCell::new(Vec::new()) };
    }

    pub(super) fn nach_dem_nullen(b: &[u8]) {
        let _ = GESEHEN.try_with(|g| g.borrow_mut().push(b.to_vec()));
    }

    #[test]
    fn der_drop_nullt_jedes_byte() {
        GESEHEN.with(|g| g.borrow_mut().clear());
        drop(Geheim::neu(b"hgx9-wert".to_vec()));
        let gesehen = GESEHEN.with(|g| g.borrow_mut().split_off(0));
        assert_eq!(gesehen, vec![vec![0u8; 9]]);
    }

    #[test]
    fn debug_zeigt_keinen_wert() {
        let g = Geheim::neu(b"hgx9-wert".to_vec());
        assert_eq!(format!("{g:?}"), "Geheim(..)");
        assert_eq!(g.bytes(), b"hgx9-wert");
    }
}
