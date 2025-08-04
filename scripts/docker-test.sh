#!/bin/bash

echo "=== P2P Sync Docker Test ==="
echo

# コンテナをクリーンアップ
echo "Cleaning up existing containers..."
docker compose down -v 2>/dev/null
docker compose -f docker-compose.dev.yml down -v 2>/dev/null

# ビルド
echo "Building Docker image..."
docker compose build

# コンテナを起動
echo "Starting containers..."
docker compose up -d

# 少し待つ
echo "Waiting for nodes to start..."
sleep 5

# ノード情報を表示
echo
echo "=== Node Information ==="
docker compose ps

# Node1でデータを追加
echo
echo "=== Adding data on Node 1 ==="
docker exec -it p2p-node1 sh -c 'echo -e "add username alice\nadd server_ip 192.168.1.100\nlist" | p2p-sync start --port 4001' &

# 少し待つ
sleep 3

# Node2でデータを確認
echo
echo "=== Checking data on Node 2 ==="
docker exec -it p2p-node2 sh -c 'echo -e "list\nget username\nget server_ip" | p2p-sync start --port 4002' &

# 結果を待つ
sleep 5

echo
echo "=== Test completed ==="
echo "To interact with nodes manually:"
echo "  Node 1: docker exec -it p2p-node1 p2p-sync start --port 4001"
echo "  Node 2: docker exec -it p2p-node2 p2p-sync start --port 4002"
echo "  Node 3: docker exec -it p2p-node3 p2p-sync start --port 4003"
echo
echo "To stop containers: docker compose down"