//! EVA-Guard — Rust Monitoring Engine.
//!
//! Collects security-related observations from a Geth node.
//! It records observations only; it never declares that an attack occurred.

pub mod collectors;
pub mod config;
pub mod geth;
pub mod models;
pub mod monitoring;
pub mod storage;
