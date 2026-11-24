//! The error of the start: an exit code and one line of text (contract section 2).

use std::fmt;

/// Why the service could not start. [`StartFehler::code`] is the exit code: 2 start file, 3 rights, 4 child, 1 socket.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StartFehler {
    /// The exit code of the program.
    pub code: i32,
    /// One line of text without a line break.
    pub text: String,
}

impl StartFehler {
    /// A start error; a line break in the text becomes a space, so that stderr gets exactly one line.
    #[must_use]
    pub fn neu(code: i32, text: &str) -> Self {
        Self {
            code,
            text: text.replace(['\n', '\r'], " "),
        }
    }
}

impl fmt::Display for StartFehler {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "wb-dienst: {}", self.text)
    }
}

impl std::error::Error for StartFehler {}

#[cfg(test)]
mod tests {
    use super::StartFehler;

    #[test]
    fn eine_zeile() {
        let f = StartFehler::neu(3, "a\nb\rc");
        assert_eq!(f.code, 3);
        assert_eq!(f.text, "a b c");
        assert_eq!(f.to_string(), "wb-dienst: a b c");
    }
}
