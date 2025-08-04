# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0] - 2025-01-04

### Added
- Initial release of P2P Sync
- Peer-to-peer synchronization using libp2p
- Key-value storage with SQLite backend
- Automatic peer discovery via mDNS
- Distributed hash table (DHT) using Kademlia
- Gossipsub protocol for message propagation
- File system monitoring with automatic sync
- Security features:
  - IP-based rate limiting
  - Connection limits per IP
  - Peer allowlist/denylist
- Cross-platform support (Linux, macOS, Windows)
- Daemon mode for background operation
- Comprehensive unit test suite
- Docker support for containerized deployment

### Features
- **Real-time Synchronization**: Changes are propagated to all connected peers immediately
- **Conflict Resolution**: Timestamp-based last-write-wins strategy
- **Resilient Network**: Automatic peer discovery and reconnection
- **Secure Communication**: Noise protocol for encrypted connections
- **Scalable Architecture**: DHT-based routing for efficient peer discovery

### Technical Details
- Built with Rust for performance and safety
- Uses tokio for async runtime
- SQLite for local storage
- libp2p for networking stack

### Known Limitations
- Maximum file size limited by available memory
- No built-in backup/restore functionality yet
- Single storage directory per instance