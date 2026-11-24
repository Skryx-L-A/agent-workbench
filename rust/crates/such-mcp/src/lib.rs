//! `such-mcp`: a thin MCP tool over stdio (contract 4, section "MCP-Werkzeug im Einschluss").
//!
//! It hands the question of the agent to the search socket of its session and returns the answer unchanged.
//! It knows neither the vault nor the settings nor the log. It reads no environment variable, writes no file,
//! keeps no cache and opens no network connection; the only connection is the Unix socket of the session.

pub mod aufruf;
pub mod mac;
pub mod rpc;
pub mod suche;
pub mod zeilen;
