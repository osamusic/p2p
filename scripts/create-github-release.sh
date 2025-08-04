#!/bin/bash

# Create GitHub release with all artifacts
# Requires gh CLI tool to be installed and authenticated

set -e

PROJECT_NAME="p2p-sync"
VERSION="0.1.0"
RELEASE_DIR="release"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Check if gh CLI is available
if ! command -v gh &> /dev/null; then
    echo -e "${RED}GitHub CLI (gh) is not installed. Please install it first.${NC}"
    echo "See: https://cli.github.com/"
    exit 1
fi

# Check if we're in a git repository
if ! git rev-parse --git-dir > /dev/null 2>&1; then
    echo -e "${RED}Not in a git repository${NC}"
    exit 1
fi

# Check if release directory exists
if [ ! -d "$RELEASE_DIR" ]; then
    echo -e "${RED}Release directory not found. Run build-release.sh first.${NC}"
    exit 1
fi

echo -e "${GREEN}Creating GitHub release for $PROJECT_NAME v$VERSION...${NC}"

# Create release notes
cat > /tmp/release-notes.md << EOF
# P2P Sync v${VERSION}

## What's New

This is the initial release of P2P Sync - a peer-to-peer file synchronization system built with Rust and libp2p.

### Features

- **Real-time Synchronization**: Automatic file synchronization across peers
- **Peer Discovery**: Automatic peer discovery using mDNS and DHT
- **Security**: Rate limiting, connection limits, and encrypted communication
- **Cross-platform**: Support for Linux, macOS, and Windows
- **Containerized**: Docker support for easy deployment

### Installation

#### Quick Installation

Download the appropriate binary for your platform and run the installation script:

**Linux:**
\`\`\`bash
tar xzf p2p-sync-${VERSION}-linux-x86_64.tar.gz
cd p2p-sync-${VERSION}-linux-x86_64
sudo ./install.sh
\`\`\`

**macOS:**
\`\`\`bash
tar xzf p2p-sync-${VERSION}-macos-x86_64.tar.gz
cd p2p-sync-${VERSION}-macos-x86_64
sudo ./install.sh
\`\`\`

**Windows:**
1. Extract \`p2p-sync-${VERSION}-windows-x86_64.zip\`
2. Run \`install.bat\` as Administrator

#### Docker

\`\`\`bash
docker run -d \\
  --name p2p-sync \\
  -v \$(pwd)/data:/data \\
  -v \$(pwd)/config:/config \\
  -p 4001:4001 \\
  p2p-sync:${VERSION}
\`\`\`

### Documentation

- [Installation Guide](INSTALL.md)
- [Configuration Reference](README.md)
- [Docker Deployment](release/docker/README.md)

### Technical Details

- Built with Rust 2021 edition
- Uses libp2p for networking
- SQLite for local storage
- Tokio for async runtime

### Known Issues

- Large files (>1GB) may cause memory issues
- Windows firewall may block connections by default

## Files

### Binary Packages
- \`p2p-sync-${VERSION}-linux-x86_64.tar.gz\` - Linux x86_64 binary
- \`p2p-sync-${VERSION}-linux-aarch64.tar.gz\` - Linux ARM64 binary  
- \`p2p-sync-${VERSION}-macos-x86_64.tar.gz\` - macOS Intel binary
- \`p2p-sync-${VERSION}-macos-aarch64.tar.gz\` - macOS Apple Silicon binary
- \`p2p-sync-${VERSION}-windows-x86_64.zip\` - Windows x86_64 binary

### Source Code
- \`p2p-sync-${VERSION}-source.tar.gz\` - Source code archive

### Docker Images
- \`p2p-sync-${VERSION}-docker.tar.gz\` - Docker image for offline installation

### Checksums
- \`SHA256SUMS\` - SHA256 checksums for all files

## What's Next

- [ ] Web UI for configuration and monitoring
- [ ] Built-in backup and restore functionality
- [ ] Support for larger files with streaming
- [ ] Plugin system for custom sync rules
EOF

# Create the release
echo -e "${GREEN}Creating GitHub release...${NC}"
gh release create "v${VERSION}" \
    --title "P2P Sync v${VERSION}" \
    --notes-file /tmp/release-notes.md \
    --draft

# Upload all release artifacts
echo -e "${GREEN}Uploading release artifacts...${NC}"

# Upload binary packages
for file in $RELEASE_DIR/*.tar.gz $RELEASE_DIR/*.zip; do
    if [ -f "$file" ]; then
        echo -e "${YELLOW}Uploading $(basename $file)...${NC}"
        gh release upload "v${VERSION}" "$file"
    fi
done

# Upload checksums
if [ -f "$RELEASE_DIR/SHA256SUMS" ]; then
    echo -e "${YELLOW}Uploading checksums...${NC}"
    gh release upload "v${VERSION}" "$RELEASE_DIR/SHA256SUMS"
fi

# Upload Docker image if it exists
if [ -f "$RELEASE_DIR/docker/p2p-sync-${VERSION}-docker.tar.gz" ]; then
    echo -e "${YELLOW}Uploading Docker image...${NC}"
    gh release upload "v${VERSION}" "$RELEASE_DIR/docker/p2p-sync-${VERSION}-docker.tar.gz"
fi

# Clean up temporary files
rm -f /tmp/release-notes.md

echo -e "${GREEN}GitHub release created successfully!${NC}"
echo -e "${YELLOW}Release URL: $(gh release view v${VERSION} --web)${NC}"
echo -e "${YELLOW}Note: Release is in draft mode. Edit and publish when ready.${NC}"