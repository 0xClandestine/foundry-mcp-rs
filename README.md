# Foundry MCP Server

A Model Context Protocol (MCP) server that provides access to all Foundry CLI tools (forge, cast, anvil, chisel) through a unified interface, plus blockchain RPC discovery and token information.

## Features

- 🔧 **177 Tools**: Complete access to all Foundry commands + blockchain RPC discovery + token information
- 🚀 **Fast**: Minimal overhead, shells out to native Foundry binaries
- 📋 **Full Schema Support**: Handles positionals, options, and flags
- 🔌 **MCP Protocol**: Standard stdio-based MCP server implementation
- 🌐 **RPC Discovery**: Query 2400+ blockchain networks and their RPC endpoints from chainlist.org
- 🔍 **Chain Search**: Find networks by name, symbol, or chain ID
- 📊 **Network Info**: Access faucets, explorers, and network metadata
- 🪙 **Token Information**: Search and discover ERC20 tokens across Ethereum and L2 chains
- 🔎 **Token Search**: Find tokens by name, symbol, or contract address
- 🌉 **Bridge Info**: Access cross-chain token bridge information
- 🔒 **Security**: Configurable forbidden commands and flags to prevent dangerous operations

## Available Tools

### Foundry Tools (170)
- **forge** (44): build, test, script, verify, coverage, snapshot, init, config, etc.
- **cast** (119): call, send, receipt, wallet, storage, decode, block, tx, etc.
- **anvil** (1): local Ethereum development node
- **chisel** (6): Solidity REPL

### Blockchain RPC Tools (3)

**`search_rpc_url`** - Search for RPC endpoints for any chain with filtering (open-source, no-tracking, websocket/http, limit)  
**`search_chains`** - Search networks by name, symbol, or chain ID  
**`list_popular_chains`** - Quick access to popular networks (Ethereum, Polygon, Arbitrum, etc.)

### Token Information Tools (4)

**`search_tokens`** - Search for tokens by name or symbol across all supported chains  
**`get_token_by_address`** - Get token information by contract address  
**`list_chain_tokens`** - List all tokens available on a specific blockchain network  
**`list_supported_chains`** - List all blockchain networks supported by the token list

**Supported Chains:**
- Ethereum, Optimism, Base, Sepolia (testnet)
- Optimism Sepolia, Base Sepolia (testnets)
- Mode, Lisk, Redstone, Metal L2, Celo
- And more L2 networks

## Installation

### Prerequisites

Foundry must be installed and available in PATH:

```bash
curl -L https://foundry.paradigm.xyz | bash
foundryup
```

### Build

```bash
cargo build --release
```

The compiled binary will be at `./target/release/foundry-mcp-server`

## Usage

### Running the Server

The server communicates via stdin/stdout using the MCP protocol:

```bash
foundry-mcp
```

### Claude Desktop Config

Add to `~/Library/Application Support/Claude/claude_desktop_config.json`:

**Without configuration (no restrictions)**:
```json
{
  "mcpServers": {
    "foundry": {
      "command": "/path/to/foundry-mcp-rs/target/release/foundry-mcp"
    }
  }
}
```

**With custom configuration**:
```json
{
  "mcpServers": {
    "foundry": {
      "command": "/path/to/foundry-mcp-rs/target/release/foundry-mcp",
      "args": ["--config", "/path/to/your/config.json"]
    }
  }
}
```

## Configuration

The server supports configurable security restrictions to prevent dangerous operations. This is especially important when exposing Foundry tools through an AI assistant.

**How it works**: Forbidden commands and flags are filtered out from the tool schema during initialization. This means the AI assistant won't even see these tools/flags - they simply won't appear in the available tools list. This is more secure than runtime validation since it follows the principle of least privilege.

### Configuration Sources

The server loads configuration from:

1. **CLI flag** `--config path/to/config.json` (explicit path)
2. **Default location** at `~/.foundry-mcp-config.json`
3. **No restrictions** if no config is found

### Configuration Format

```json
{
  "forbidden_commands": ["anvil", "forge_script"],
  "forbidden_flags": ["broadcast", "private-key", "mnemonic"],
  "allow_dangerous": false
}
```

### Configuration Options

- **`forbidden_commands`**: Array of command names to block (e.g., `"anvil"`, `"forge_script"`, `"cast_send"`)
- **`forbidden_flags`**: Array of flag names to block (e.g., `"broadcast"`, `"private-key"`, `"mnemonic"`)
- **`allow_dangerous`**: Boolean to control hardcoded dangerous restrictions
  - `false` (default): Automatically adds hardcoded dangerous commands/flags to your forbidden lists
  - `true`: Only uses your explicitly configured forbidden lists

### Hardcoded Dangerous Restrictions

When `allow_dangerous: false` (the default), the following hardcoded restrictions are **automatically** merged with your config:

**Dangerous Commands:**
- `anvil` - Local Ethereum node

**Dangerous Flags:**
- `broadcast` - Sends transactions to networks
- `private-key` - Uses private keys directly
- `mnemonic` - Uses mnemonic phrases directly
- `legacy` - Legacy transaction types
- `unlock` - Unlocks accounts

**Note:** Your custom `forbidden_commands` and `forbidden_flags` are merged with these hardcoded values, so you can add additional restrictions without needing to repeat the defaults.

### Usage Examples

**Run with default safety restrictions**:
```bash
foundry-mcp
```

**Run with custom config**:
```bash
foundry-mcp --config /path/to/config.json
```

**Copy config to default location** (updates systemwide defaults):
```bash
cp config.safe.json ~/.foundry-mcp-config.json
foundry-mcp
```

**Show help**:
```bash
foundry-mcp --help
```

## How It Works

1. Loads Foundry CLI schemas from `schemas.json`
2. Exposes 170 Foundry tools + 3 RPC discovery tools + 4 token information tools via MCP
3. Shells out to native Foundry binaries for execution
4. Fetches blockchain RPC data from chainlist.org (cached)
5. Fetches token information from Optimism token list (cached)

## Architecture

- **No Foundry deps**: Shells out to native binaries (avoids 800+ transitive deps)
- **Modular**: Clean separation (foundry.rs, chainlist.rs, tokenlist.rs, server.rs)
- **Cached data**: In-memory cache for both chainlist.org and token list APIs
- **MCP resources**: 
  - `chainlist://all` - 2400+ blockchain networks database
  - `tokenlist://all` - ERC20 tokens across Ethereum and L2 chains

## License

MIT or Apache-2.0 (same as Foundry)

## Related Projects

- [Foundry](https://github.com/foundry-rs/foundry) - Fast, portable and modular toolkit for Ethereum development
- [MCP](https://modelcontextprotocol.io/) - Model Context Protocol specification
- [rmcp](https://github.com/modelcontextprotocol/rust-sdk) - Official Rust SDK for MCP

