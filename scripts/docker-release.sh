#!/bin/bash

# Build and package Docker images for p2p-sync release

set -e

PROJECT_NAME="p2p-sync"
VERSION="0.1.0"
REGISTRY="ghcr.io/yourusername"  # Change this to your registry

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo -e "${GREEN}Building Docker images for $PROJECT_NAME v$VERSION...${NC}"

# Build multi-architecture images
echo -e "${GREEN}Building multi-platform Docker image...${NC}"

# Create and use a new builder instance
docker buildx create --name p2p-sync-builder --use || docker buildx use p2p-sync-builder

# Build for multiple platforms
docker buildx build \
    --platform linux/amd64,linux/arm64 \
    --tag $REGISTRY/$PROJECT_NAME:$VERSION \
    --tag $REGISTRY/$PROJECT_NAME:latest \
    --push \
    .

# Build local image for testing
echo -e "${GREEN}Building local Docker image for testing...${NC}"
docker build -t $PROJECT_NAME:$VERSION .
docker tag $PROJECT_NAME:$VERSION $PROJECT_NAME:latest

# Save Docker images as tar files for offline distribution
echo -e "${GREEN}Saving Docker images for offline distribution...${NC}"
mkdir -p release/docker

docker save $PROJECT_NAME:$VERSION | gzip > release/docker/$PROJECT_NAME-$VERSION-docker.tar.gz

# Create Docker Compose files for easy deployment
echo -e "${GREEN}Creating Docker Compose files...${NC}"

cat > release/docker/docker-compose.yml << EOF
version: '3.8'

services:
  p2p-sync:
    image: $PROJECT_NAME:$VERSION
    container_name: p2p-sync
    restart: unless-stopped
    volumes:
      - ./data:/data
      - ./config:/config
    ports:
      - "4001:4001"
    environment:
      - RUST_LOG=info
      - P2P_SYNC_CONFIG=/config/config.toml
    networks:
      - p2p-sync-network

networks:
  p2p-sync-network:
    driver: bridge

volumes:
  p2p-sync-data:
    driver: local
EOF

cat > release/docker/docker-compose.prod.yml << EOF
version: '3.8'

services:
  p2p-sync:
    image: $REGISTRY/$PROJECT_NAME:$VERSION
    container_name: p2p-sync
    restart: unless-stopped
    volumes:
      - p2p-sync-data:/data
      - ./config:/config:ro
    ports:
      - "4001:4001"
    environment:
      - RUST_LOG=warn
      - P2P_SYNC_CONFIG=/config/config.toml
    networks:
      - p2p-sync-network
    healthcheck:
      test: ["CMD", "/app/p2p-sync", "status"]
      interval: 30s
      timeout: 10s
      retries: 3
      start_period: 40s

networks:
  p2p-sync-network:
    driver: bridge

volumes:
  p2p-sync-data:
    driver: local
EOF

# Create development Docker Compose with additional services
cat > release/docker/docker-compose.dev.yml << EOF
version: '3.8'

services:
  p2p-sync-node1:
    image: $PROJECT_NAME:$VERSION
    container_name: p2p-sync-node1
    volumes:
      - ./data/node1:/data
      - ./config/node1:/config
    ports:
      - "4001:4001"
    environment:
      - RUST_LOG=debug
      - P2P_SYNC_NODE_ID=node1
    networks:
      - p2p-sync-network

  p2p-sync-node2:
    image: $PROJECT_NAME:$VERSION
    container_name: p2p-sync-node2
    volumes:
      - ./data/node2:/data
      - ./config/node2:/config
    ports:
      - "4002:4001"
    environment:
      - RUST_LOG=debug
      - P2P_SYNC_NODE_ID=node2
    networks:
      - p2p-sync-network

  p2p-sync-node3:
    image: $PROJECT_NAME:$VERSION
    container_name: p2p-sync-node3
    volumes:
      - ./data/node3:/data
      - ./config/node3:/config
    ports:
      - "4003:4001"
    environment:
      - RUST_LOG=debug
      - P2P_SYNC_NODE_ID=node3
    networks:
      - p2p-sync-network

networks:
  p2p-sync-network:
    driver: bridge
EOF

# Create deployment documentation
cat > release/docker/README.md << EOF
# Docker Deployment

This directory contains Docker images and compose files for deploying P2P Sync.

## Quick Start

### Using Docker Compose

1. Create config directory:
   \`\`\`bash
   mkdir -p config data
   cp ../config/config.toml.example config/config.toml
   \`\`\`

2. Start the service:
   \`\`\`bash
   docker-compose up -d
   \`\`\`

### Using Pre-built Image

\`\`\`bash
docker run -d \\
  --name p2p-sync \\
  -v \$(pwd)/data:/data \\
  -v \$(pwd)/config:/config \\
  -p 4001:4001 \\
  $PROJECT_NAME:$VERSION
\`\`\`

### Loading Offline Image

\`\`\`bash
# Load the image
docker load < $PROJECT_NAME-$VERSION-docker.tar.gz

# Run the container
docker run -d --name p2p-sync -p 4001:4001 $PROJECT_NAME:$VERSION
\`\`\`

## Available Files

- \`docker-compose.yml\` - Basic single-node deployment
- \`docker-compose.prod.yml\` - Production deployment with health checks
- \`docker-compose.dev.yml\` - Development setup with 3 nodes
- \`$PROJECT_NAME-$VERSION-docker.tar.gz\` - Offline Docker image

## Configuration

Mount your configuration file to \`/config/config.toml\` in the container.

## Data Persistence

Data is stored in \`/data\` inside the container. Mount a volume to persist data across container restarts.

## Networks

All compose files create an isolated network for P2P Sync services.

## Health Checks

The production compose file includes health checks to monitor service status.
EOF

# Create Kubernetes deployment files
echo -e "${GREEN}Creating Kubernetes deployment files...${NC}"
mkdir -p release/k8s

cat > release/k8s/deployment.yaml << EOF
apiVersion: apps/v1
kind: Deployment
metadata:
  name: p2p-sync
  labels:
    app: p2p-sync
spec:
  replicas: 1
  selector:
    matchLabels:
      app: p2p-sync
  template:
    metadata:
      labels:
        app: p2p-sync
    spec:
      containers:
      - name: p2p-sync
        image: $REGISTRY/$PROJECT_NAME:$VERSION
        ports:
        - containerPort: 4001
        env:
        - name: RUST_LOG
          value: "info"
        - name: P2P_SYNC_CONFIG
          value: "/config/config.toml"
        volumeMounts:
        - name: config-volume
          mountPath: /config
        - name: data-volume
          mountPath: /data
        livenessProbe:
          exec:
            command:
            - /app/p2p-sync
            - status
          initialDelaySeconds: 30
          periodSeconds: 30
        readinessProbe:
          exec:
            command:
            - /app/p2p-sync
            - status
          initialDelaySeconds: 5
          periodSeconds: 10
      volumes:
      - name: config-volume
        configMap:
          name: p2p-sync-config
      - name: data-volume
        persistentVolumeClaim:
          claimName: p2p-sync-data
---
apiVersion: v1
kind: Service
metadata:
  name: p2p-sync-service
spec:
  selector:
    app: p2p-sync
  ports:
  - protocol: TCP
    port: 4001
    targetPort: 4001
  type: LoadBalancer
---
apiVersion: v1
kind: PersistentVolumeClaim
metadata:
  name: p2p-sync-data
spec:
  accessModes:
  - ReadWriteOnce
  resources:
    requests:
      storage: 10Gi
---
apiVersion: v1
kind: ConfigMap
metadata:
  name: p2p-sync-config
data:
  config.toml: |
    [network]
    listen_address = "/ip4/0.0.0.0/tcp/4001"
    enable_mdns = true
    enable_kad = true

    [storage]
    data_dir = "/data"

    [security]
    max_connections_per_ip = 5
    rate_limit_window_secs = 60
    rate_limit_max_requests = 100

    [logging]
    level = "info"
EOF

echo -e "${GREEN}Docker release build complete!${NC}"
echo -e "${YELLOW}Docker images and compose files created in release/docker/${NC}"
echo -e "${YELLOW}Kubernetes deployment files created in release/k8s/${NC}"