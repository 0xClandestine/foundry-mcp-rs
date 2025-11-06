# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0] - 2025-11-06

### Added
- Initial release of Foundry MCP Server
- 170 Foundry tools (forge, cast, anvil, chisel)
- 3 blockchain RPC discovery tools (get_rpc, search_chains, list_popular_chains)
- 4 token information tools (search_tokens, get_token_by_address, list_chain_tokens, list_supported_chains)
- Configurable security restrictions via JSON config
- Support for forbidden commands and flags
- Hardcoded dangerous operation restrictions
- In-memory caching for chainlist.org and token list APIs
- MCP resources for blockchain and token data
- Comprehensive documentation and configuration examples

### Features
- Complete access to all Foundry CLI commands
- RPC discovery from 2400+ blockchain networks
- Token information across Ethereum and L2 chains
- Security-first design with configurable restrictions
- Fast execution by shelling out to native Foundry binaries

[0.1.0]: https://github.com/0xclandestine/foundry-mcp-rs/releases/tag/v0.1.0

