#!/bin/bash

# Build release packages for p2p-sync
# This script creates release builds for multiple platforms

set -e

PROJECT_NAME="p2p-sync"
VERSION="0.1.0"
RELEASE_DIR="release"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo -e "${GREEN}Building $PROJECT_NAME v$VERSION release packages...${NC}"

# Clean previous releases
echo -e "${YELLOW}Cleaning previous release directory...${NC}"
rm -rf $RELEASE_DIR
mkdir -p $RELEASE_DIR

# Build for current platform
echo -e "${GREEN}Building for current platform...${NC}"
cargo build --release

# Function to create release package
create_package() {
    local target=$1
    local platform=$2
    local extension=$3
    
    echo -e "${GREEN}Creating package for $platform...${NC}"
    
    local pkg_dir="$RELEASE_DIR/${PROJECT_NAME}-${VERSION}-${platform}"
    mkdir -p "$pkg_dir"
    
    # Copy binary
    if [ -f "target/$target/release/$PROJECT_NAME" ]; then
        cp "target/$target/release/$PROJECT_NAME" "$pkg_dir/"
    elif [ -f "target/$target/release/${PROJECT_NAME}.exe" ]; then
        cp "target/$target/release/${PROJECT_NAME}.exe" "$pkg_dir/"
    else
        echo -e "${RED}Binary not found for $platform${NC}"
        return 1
    fi
    
    # Copy documentation
    cp README.md "$pkg_dir/"
    
    # Create config directory and sample config
    mkdir -p "$pkg_dir/config"
    
    # Create sample configuration
    cat > "$pkg_dir/config/config.toml.example" << EOF
# P2P Sync Configuration

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
enable_access_control = true

[logging]
level = "info"
EOF
    
    # Create systemd service file for Linux
    if [[ "$platform" == *"linux"* ]]; then
        cat > "$pkg_dir/p2p-sync.service" << EOF
[Unit]
Description=P2P Sync Service
After=network.target

[Service]
Type=simple
ExecStart=/usr/local/bin/p2p-sync start
Restart=on-failure
RestartSec=10
User=p2psync
Group=p2psync

[Install]
WantedBy=multi-user.target
EOF
    fi
    
    # Create installation script
    if [[ "$platform" == *"windows"* ]]; then
        cat > "$pkg_dir/install.bat" << 'EOF'
@echo off
echo Installing P2P Sync...

REM Create installation directory
mkdir "%PROGRAMFILES%\p2p-sync" 2>nul

REM Copy files
copy p2p-sync.exe "%PROGRAMFILES%\p2p-sync\"
copy config\config.toml.example "%PROGRAMFILES%\p2p-sync\config.toml"

echo Installation complete!
echo Please add %PROGRAMFILES%\p2p-sync to your PATH
pause
EOF
    else
        cat > "$pkg_dir/install.sh" << 'EOF'
#!/bin/bash
set -e

echo "Installing P2P Sync..."

# Check if running as root
if [ "$EUID" -ne 0 ]; then 
   echo "Please run as root (use sudo)"
   exit 1
fi

# Create user and group
useradd -r -s /bin/false p2psync 2>/dev/null || true

# Install binary
install -m 755 p2p-sync /usr/local/bin/

# Create config directory
mkdir -p /etc/p2p-sync
install -m 644 config/config.toml.example /etc/p2p-sync/config.toml

# Install systemd service (if available)
if [ -f p2p-sync.service ] && systemctl --version >/dev/null 2>&1; then
    install -m 644 p2p-sync.service /etc/systemd/system/
    systemctl daemon-reload
    echo "Systemd service installed. Enable with: systemctl enable p2p-sync"
fi

# Create data directory
mkdir -p /var/lib/p2p-sync
chown p2psync:p2psync /var/lib/p2p-sync

echo "Installation complete!"
EOF
        chmod +x "$pkg_dir/install.sh"
    fi
    
    # Create archive
    cd $RELEASE_DIR
    if [[ "$extension" == "zip" ]]; then
        zip -r "${PROJECT_NAME}-${VERSION}-${platform}.zip" "$(basename "$pkg_dir")"
    else
        tar czf "${PROJECT_NAME}-${VERSION}-${platform}.tar.gz" "$(basename "$pkg_dir")"
    fi
    cd ..
    
    # Clean up directory
    rm -rf "$pkg_dir"
}

# Build for specific targets if rust targets are installed
if command -v rustup &> /dev/null; then
    # Linux x86_64
    if rustup target list | grep -q "x86_64-unknown-linux-gnu (installed)"; then
        echo -e "${GREEN}Building for Linux x86_64...${NC}"
        cargo build --release --target x86_64-unknown-linux-gnu
        create_package "x86_64-unknown-linux-gnu" "linux-x86_64" "tar.gz"
    fi
    
    # Linux ARM64
    if rustup target list | grep -q "aarch64-unknown-linux-gnu (installed)"; then
        echo -e "${GREEN}Building for Linux ARM64...${NC}"
        cargo build --release --target aarch64-unknown-linux-gnu
        create_package "aarch64-unknown-linux-gnu" "linux-aarch64" "tar.gz"
    fi
    
    # Windows x86_64
    if rustup target list | grep -q "x86_64-pc-windows-gnu (installed)"; then
        echo -e "${GREEN}Building for Windows x86_64...${NC}"
        cargo build --release --target x86_64-pc-windows-gnu
        create_package "x86_64-pc-windows-gnu" "windows-x86_64" "zip"
    fi
    
    # macOS x86_64
    if rustup target list | grep -q "x86_64-apple-darwin (installed)"; then
        echo -e "${GREEN}Building for macOS x86_64...${NC}"
        cargo build --release --target x86_64-apple-darwin
        create_package "x86_64-apple-darwin" "macos-x86_64" "tar.gz"
    fi
    
    # macOS ARM64
    if rustup target list | grep -q "aarch64-apple-darwin (installed)"; then
        echo -e "${GREEN}Building for macOS ARM64...${NC}"
        cargo build --release --target aarch64-apple-darwin
        create_package "aarch64-apple-darwin" "macos-aarch64" "tar.gz"
    fi
else
    # Build only for current platform
    echo -e "${YELLOW}Building only for current platform (rustup not found)${NC}"
    CURRENT_TARGET=$(rustc -vV | sed -n 's|host: ||p')
    
    case "$CURRENT_TARGET" in
        *linux*)
            create_package "release" "linux-$(uname -m)" "tar.gz"
            ;;
        *darwin*)
            create_package "release" "macos-$(uname -m)" "tar.gz"
            ;;
        *windows*)
            create_package "release" "windows-$(uname -m)" "zip"
            ;;
        *)
            echo -e "${RED}Unknown platform: $CURRENT_TARGET${NC}"
            ;;
    esac
fi

# Create source archive
echo -e "${GREEN}Creating source archive...${NC}"
git archive --format=tar.gz --prefix="${PROJECT_NAME}-${VERSION}/" -o "$RELEASE_DIR/${PROJECT_NAME}-${VERSION}-source.tar.gz" HEAD

# Generate checksums
echo -e "${GREEN}Generating checksums...${NC}"
cd $RELEASE_DIR
sha256sum * > SHA256SUMS
cd ..

echo -e "${GREEN}Release packages created in $RELEASE_DIR directory:${NC}"
ls -la $RELEASE_DIR/

echo -e "${GREEN}Build complete!${NC}"