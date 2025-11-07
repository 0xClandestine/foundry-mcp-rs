//! Foundry MCP Server
//!
//! A Model Context Protocol (MCP) server that provides access to all Foundry CLI tools
//! (forge, cast, anvil, chisel) through a unified interface, plus blockchain RPC discovery
//! via chainlist.org and token information via the Optimism token list.

pub mod chainlist;
pub mod config;
pub mod context;
pub mod conversion;
pub mod foundry;
pub mod handlers;
pub mod schema;
pub mod server;
pub mod sessions;
pub mod tokenlist;

pub use server::FoundryMcpHandler;
