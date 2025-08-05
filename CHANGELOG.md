# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- Automated key distribution system with 5 message types
- Simple trust chain system with peer recommendations
- Ed25519 digital signature verification for all messages
- SQLite-based whitelist management with trust extensions
- 8 new interactive CLI commands for key/trust management
- Cross-platform release automation with GitHub Actions

### Enhanced
- Complete security overhaul with signature-based authentication
- Trust-based access control with recommendation system
- Comprehensive documentation reorganization
- Enhanced CI/CD pipeline with multi-platform builds

### Technical
- New modules: `crypto.rs`, `key_distribution.rs`, `whitelist.rs`
- P2PMessage unified messaging system
- Extended whitelist with trust chain support
- Complete test coverage for new functionality

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