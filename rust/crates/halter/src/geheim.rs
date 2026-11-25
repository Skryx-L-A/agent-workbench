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
    }
}
