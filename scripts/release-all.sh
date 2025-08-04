#!/bin/bash

# Complete release pipeline for p2p-sync
# This script runs all release tasks in sequence

set -e

PROJECT_NAME="p2p-sync"
VERSION="0.1.0"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

echo -e "${BLUE}======================================${NC}"
echo -e "${BLUE}    P2P Sync Complete Release Build   ${NC}"
echo -e "${BLUE}              v${VERSION}                ${NC}"
echo -e "${BLUE}======================================${NC}"

# Function to run a step with error handling
run_step() {
    local step_name=$1
    local script_path=$2
    
    echo -e "\n${GREEN}>>> Step: ${step_name}${NC}"
    
    if [ -f "$script_path" ]; then
        if bash "$script_path"; then
            echo -e "${GREEN}✓ ${step_name} completed successfully${NC}"
        else
            echo -e "${RED}✗ ${step_name} failed${NC}"
            exit 1
        fi
    else
        echo -e "${RED}✗ Script not found: ${script_path}${NC}"
        exit 1
    fi
}

# Pre-flight checks
echo -e "\n${YELLOW}Running pre-flight checks...${NC}"

# Check if we're in the project root
if [ ! -f "Cargo.toml" ]; then
    echo -e "${RED}Error: Not in project root directory${NC}"
    exit 1
fi

# Check if all required tools are available
REQUIRED_TOOLS=("cargo" "git" "tar" "gzip")
for tool in "${REQUIRED_TOOLS[@]}"; do
    if ! command -v "$tool" &> /dev/null; then
        echo -e "${RED}Error: Required tool '$tool' not found${NC}"
        exit 1
    fi
done

# Run tests first
echo -e "\n${GREEN}>>> Running test suite${NC}"
if cargo test --lib; then
    echo -e "${GREEN}✓ All tests passed${NC}"
else
    echo -e "${RED}✗ Tests failed. Aborting release.${NC}"
    exit 1
fi

# Check if working directory is clean
if [ -n "$(git status --porcelain)" ]; then
    echo -e "${YELLOW}Warning: Working directory has uncommitted changes${NC}"
    read -p "Continue anyway? [y/N] " -n 1 -r
    echo
    if [[ ! $REPLY =~ ^[Yy]$ ]]; then
        echo -e "${RED}Aborting release${NC}"
        exit 1
    fi
fi

# Create scripts directory if it doesn't exist
mkdir -p scripts

# Step 1: Build release packages
run_step "Building release packages" "scripts/build-release.sh"

# Step 2: Build Docker images
echo -e "\n${YELLOW}Do you want to build Docker images? [y/N]${NC}"
read -p "" -n 1 -r
echo
if [[ $REPLY =~ ^[Yy]$ ]]; then
    run_step "Building Docker images" "scripts/docker-release.sh"
else
    echo -e "${YELLOW}Skipping Docker image build${NC}"
fi

# Step 3: Create GitHub release
echo -e "\n${YELLOW}Do you want to create a GitHub release? [y/N]${NC}"
read -p "" -n 1 -r
echo
if [[ $REPLY =~ ^[Yy]$ ]]; then
    if command -v gh &> /dev/null; then
        run_step "Creating GitHub release" "scripts/create-github-release.sh"
    else
        echo -e "${YELLOW}GitHub CLI not found. Skipping GitHub release creation.${NC}"
        echo -e "${YELLOW}You can create the release manually or install gh CLI.${NC}"
    fi
else
    echo -e "${YELLOW}Skipping GitHub release creation${NC}"
fi

# Summary
echo -e "\n${BLUE}======================================${NC}"
echo -e "${GREEN}     Release Build Complete!          ${NC}"
echo -e "${BLUE}======================================${NC}"

echo -e "\n${GREEN}Generated artifacts:${NC}"
if [ -d "release" ]; then
    ls -la release/
    
    if [ -d "release/docker" ]; then
        echo -e "\n${GREEN}Docker artifacts:${NC}"
        ls -la release/docker/
    fi
    
    if [ -d "release/k8s" ]; then
        echo -e "\n${GREEN}Kubernetes artifacts:${NC}"
        ls -la release/k8s/
    fi
else
    echo -e "${RED}No release directory found${NC}"
fi

echo -e "\n${GREEN}Next steps:${NC}"
echo -e "1. Test the release packages on target platforms"
echo -e "2. Verify Docker images work correctly"
echo -e "3. Update documentation if needed"
echo -e "4. Publish the GitHub release when ready"
echo -e "5. Push Docker images to registry"

echo -e "\n${BLUE}Release v${VERSION} is ready for distribution!${NC}"