//! Helpers of the tests. The fake asker is portable; the socket helpers (`unix.rs`) exist only on Unix.
#![allow(dead_code)]

use std::cell::RefCell;

use such_mcp::suche::{Befrager, Unerreichbar};

#[cfg(unix)]
mod unix;
#[cfg(unix)]
#[allow(unused_imports)]
pub use unix::{StandIn, Wegwerf};

/// A fake asker: records every request line and gives the same answer.
pub struct Attrappe {
    pub antwort: Result<String, Unerreichbar>,
    pub gefragt: RefCell<Vec<String>>,
}

impl Attrappe {
    pub fn mit(antwort: &str) -> Self {
        Self {
            antwort: Ok(antwort.to_owned()),
            gefragt: RefCell::new(Vec::new()),
        }
    }

    pub fn unerreichbar() -> Self {
        Self {
            antwort: Err(Unerreichbar),
            gefragt: RefCell::new(Vec::new()),
        }
    }
}

impl Befrager for Attrappe {
    fn frage(&self, zeile: &str) -> Result<String, Unerreichbar> {
        self.gefragt.borrow_mut().push(zeile.to_owned());
        self.antwort.clone()
    }
}
