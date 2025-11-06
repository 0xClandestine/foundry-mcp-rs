//! Foundry MCP Server
//!
//! A Model Context Protocol (MCP) server that provides access to all Foundry CLI tools
//! (forge, cast, anvil, chisel) through a unified interface, plus blockchain RPC discovery
//! via chainlist.org and token information via the Optimism token list.

pub mod chainlist;
pub mod config;
pub mod context;
pub mod foundry;
pub mod schema;
pub mod server;
pub mod tokenlist;

pub use server::FoundryMcpHandler;
