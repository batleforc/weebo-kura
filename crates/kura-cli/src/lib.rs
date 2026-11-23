//! `kura`, the reference client of RFC 0001 (§6.8): the commands, the HTTP
//! client and the vault folder. `main.rs` only parses arguments.

pub mod api;
pub mod commands;
pub mod config;
pub mod fs;
pub mod plugin_zip;
