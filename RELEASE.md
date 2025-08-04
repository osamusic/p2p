# Release Package Guide

The release package system for P2P Sync v0.1.0 has been created with comprehensive build scripts and documentation.

## Quick Start

To create a complete release package, run:

```bash
./scripts/release-all.sh
```

This will guide you through the entire release process interactively.

## Individual Scripts

### 1. Binary Release Packages

```bash
./scripts/build-release.sh
```

Creates platform-specific binary packages with:
- Cross-compiled binaries for Linux, macOS, Windows (if targets are installed)
- Installation scripts for each platform
- Configuration examples
- systemd service files (Linux)
- SHA256 checksums

**Output:** `release/` directory with `.tar.gz` and `.zip` files

### 2. Docker Images

```bash
./scripts/docker-release.sh
```

Creates Docker deployment artifacts:
- Multi-architecture Docker images (linux/amd64, linux/arm64)
- Docker Compose files for different scenarios
- Kubernetes deployment manifests
- Offline Docker image archives

**Output:** `release/docker/` and `release/k8s/` directories

### 3. GitHub Release

```bash
./scripts/create-github-release.sh
```

Creates a GitHub release with all artifacts:
- Uploads all binary packages
- Creates comprehensive release notes
- Includes checksums and documentation
- Creates draft release for review

**Requirements:** GitHub CLI (`gh`) installed and authenticated

## Generated Files

### Binary Packages
- `p2p-sync-0.1.0-linux-x86_64.tar.gz`
- `p2p-sync-0.1.0-linux-aarch64.tar.gz`
- `p2p-sync-0.1.0-macos-x86_64.tar.gz`
- `p2p-sync-0.1.0-macos-aarch64.tar.gz`
- `p2p-sync-0.1.0-windows-x86_64.zip`
- `p2p-sync-0.1.0-source.tar.gz`

### Docker Files
- `docker-compose.yml` - Basic deployment
- `docker-compose.prod.yml` - Production deployment
- `docker-compose.dev.yml` - Development with 3 nodes
- `p2p-sync-0.1.0-docker.tar.gz` - Offline Docker image

### Kubernetes Files
- `deployment.yaml` - Complete K8s deployment with service, PVC, and ConfigMap

### Documentation
- `CHANGELOG.md` - Version history and changes
- `INSTALL.md` - Comprehensive installation guide
- `README.md` files for Docker and K8s deployments

## Package Contents

Each binary package includes:
- Compiled binary for the target platform
- Installation script (`install.sh` or `install.bat`)
- Sample configuration file
- systemd service file (Linux only)
- README with basic usage instructions

## Installation Methods

### Native Binary
1. Download appropriate package for your platform
2. Extract archive
3. Run installation script with sudo/administrator privileges

### Docker
```bash
docker-compose up -d
```

### Kubernetes
```bash
kubectl apply -f release/k8s/deployment.yaml
```

## Distribution Checklist

Before distributing the release:

- [ ] Test binary packages on target platforms
- [ ] Verify Docker images work correctly
- [ ] Test installation scripts
- [ ] Verify checksums match
- [ ] Review and publish GitHub release
- [ ] Push Docker images to registry
- [ ] Update documentation if needed

## Security Notes

- All packages include SHA256 checksums for verification
- Docker images are built with minimal base images
- Installation scripts create dedicated system users
- Configuration examples include security best practices

## Support

For installation issues, refer to:
- `INSTALL.md` - Detailed installation instructions
- `CHANGELOG.md` - Known issues and limitations
- GitHub Issues - Community support