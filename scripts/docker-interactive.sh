#!/bin/bash

# インタラクティブなDockerテスト環境を起動

echo "=== P2P Sync Interactive Docker Test ==="
echo

# 既存のコンテナをクリーンアップ
docker-compose down -v 2>/dev/null

# ビルドが必要か確認
if [ ! -f "target/release/p2p-sync" ]; then
    echo "Building p2p-sync..."
    cargo build --release
fi

# Dockerイメージをビルド
echo "Building Docker image..."
docker-compose build

# 3つのターミナルウィンドウでノードを起動
echo "Starting nodes in separate windows..."
echo

# Node 1を起動
echo "Starting Node 1..."
gnome-terminal --title="P2P Node 1" -- bash -c "docker run --rm -it --name p2p-node1 --hostname node1 -p 4001:4001/tcp -p 4001:4001/udp -e RUST_LOG=info p2p-sync start --port 4001; read -p 'Press enter to close...'"

sleep 2

# Node 2を起動
echo "Starting Node 2..."
gnome-terminal --title="P2P Node 2" -- bash -c "docker run --rm -it --name p2p-node2 --hostname node2 -p 4002:4002/tcp -p 4002:4002/udp -e RUST_LOG=info p2p-sync start --port 4002; read -p 'Press enter to close...'"

sleep 2

# Node 3を起動
echo "Starting Node 3..."
gnome-terminal --title="P2P Node 3" -- bash -c "docker run --rm -it --name p2p-node3 --hostname node3 -p 4003:4003/tcp -p 4003:4003/udp -e RUST_LOG=info p2p-sync start --port 4003; read -p 'Press enter to close...'"

echo
echo "=== Nodes are starting in separate terminal windows ==="
echo
echo "Test commands:"
echo "  Node 1: add username alice"
echo "  Node 2: get username"
echo "  Node 2: add server_ip 192.168.1.100"
echo "  Node 3: list"
echo
echo "To stop all nodes, close the terminal windows or press Ctrl+C in each."