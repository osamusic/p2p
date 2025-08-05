# Installation Guide

## Quick Start

### Using Pre-built Binaries

1. Download the appropriate package for your platform from the releases page
2. Extract the archive
3. Run the installation script:
   - Linux/macOS: `sudo ./install.sh`
   - Windows: Run `install.bat` as Administrator

### Building from Source

#### Prerequisites

- Rust 1.70 or later
- Git

#### Build Steps

```bash
# Clone the repository
git clone https://github.com/yourusername/p2p-sync.git
cd p2p-sync

# Build in release mode
cargo build --release

# Binary will be in target/release/p2p-sync
```

## Platform-Specific Instructions

### Linux

#### Using the Installation Script

```bash
# Extract the package
tar xzf p2p-sync-0.1.0-linux-x86_64.tar.gz
cd p2p-sync-0.1.0-linux-x86_64

# Run installation script
sudo ./install.sh
```

#### Manual Installation

```bash
# Install binary
sudo install -m 755 p2p-sync /usr/local/bin/

# Create config directory
sudo mkdir -p /etc/p2p-sync
sudo cp config/config.toml.example /etc/p2p-sync/config.toml

# Create system user
sudo useradd -r -s /bin/false p2psync

# Create data directory
sudo mkdir -p /var/lib/p2p-sync
sudo chown p2psync:p2psync /var/lib/p2p-sync

# Install systemd service (optional)
sudo cp p2p-sync.service /etc/systemd/system/
sudo systemctl daemon-reload
sudo systemctl enable p2p-sync
```

### macOS

```bash
# Extract the package
tar xzf p2p-sync-0.1.0-macos-x86_64.tar.gz
cd p2p-sync-0.1.0-macos-x86_64

# Install binary
sudo cp p2p-sync /usr/local/bin/
sudo chmod +x /usr/local/bin/p2p-sync

# Create config directory
mkdir -p ~/.p2p-sync
cp config/config.toml.example ~/.p2p-sync/config.toml
```

### Windows

1. Extract the ZIP file to a directory (e.g., `C:\Program Files\p2p-sync`)
2. Run `install.bat` as Administrator
3. Add the installation directory to your PATH environment variable

## Configuration

The default configuration file is located at:
- Linux: `/etc/p2p-sync/config.toml`
- macOS: `~/.p2p-sync/config.toml`
- Windows: `%PROGRAMFILES%\p2p-sync\config.toml`

### Basic Configuration

```toml
[network]
listen_address = "/ip4/0.0.0.0/tcp/4001"
enable_mdns = true
enable_kad = true

[storage]
data_dir = "~/.p2p-sync/data"

[security]
max_connections_per_ip = 5
rate_limit_window_secs = 60
rate_limit_max_requests = 100
```

## Running P2P Sync

### Command Line

```bash
# Start the service
p2p-sync start

# Start in daemon mode (background)
p2p-sync start --daemon

# Specify custom config
p2p-sync start --config /path/to/config.toml

# Watch a directory for changes
p2p-sync start --watch /path/to/directory
```

### Using systemd (Linux)

```bash
# Start the service
sudo systemctl start p2p-sync

# Enable auto-start on boot
sudo systemctl enable p2p-sync

# Check status
sudo systemctl status p2p-sync

# View logs
sudo journalctl -u p2p-sync -f
```

## Docker Installation

### Using Docker Compose

```yaml
version: '3.8'
services:
  p2p-sync:
    image: p2p-sync:0.1.0
    volumes:
      - ./data:/data
      - ./config:/config
    ports:
      - "4001:4001"
    environment:
      - RUST_LOG=info
```

### Building Docker Image

```bash
# Build the image
docker build -t p2p-sync:0.1.0 .

# Run container
docker run -d \
  --name p2p-sync \
  -v $(pwd)/data:/data \
  -v $(pwd)/config:/config \
  -p 4001:4001 \
  p2p-sync:0.1.0
```

## Verification

After installation, verify that P2P Sync is working:

```bash
# Check version
p2p-sync --version

# Test configuration
p2p-sync start --dry-run

# Check connectivity
p2p-sync status
```

## Troubleshooting

### Permission Denied

If you get permission errors, ensure:
- The binary has execute permissions: `chmod +x /usr/local/bin/p2p-sync`
- The data directory is writable by the p2psync user
- The config file is readable

### Port Already in Use

If port 4001 is already in use, change it in the configuration:

```toml
[network]
listen_address = "/ip4/0.0.0.0/tcp/4002"
```

### Connection Issues

1. Check firewall settings - ensure port 4001 (or your configured port) is open
2. Verify mDNS is working on your network
3. Check logs for error messages

## Uninstallation

### Linux

```bash
# Stop and disable service
sudo systemctl stop p2p-sync
sudo systemctl disable p2p-sync

# Remove files
sudo rm /usr/local/bin/p2p-sync
sudo rm -rf /etc/p2p-sync
sudo rm -rf /var/lib/p2p-sync
sudo rm /etc/systemd/system/p2p-sync.service

# Remove user
sudo userdel p2psync
```

### macOS

```bash
rm /usr/local/bin/p2p-sync
rm -rf ~/.p2p-sync
```

### Windows

1. Delete the installation directory
2. Remove from PATH environment variable