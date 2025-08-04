.PHONY: build run-local docker-build docker-up docker-down docker-test clean

# ローカルビルド
build:
	cargo build --release

# ローカル実行
run-local:
	RUST_LOG=info cargo run --release -- start

# Dockerイメージのビルド
docker-build:
	docker compose build

# Dockerコンテナの起動
docker-up:
	docker compose up -d

# Dockerコンテナの停止
docker-down:
	docker compose down

# Dockerコンテナの停止と削除
docker-clean:
	docker compose down -v

# 開発用Docker環境（ホストネットワーク使用）
docker-dev:
	docker compose -f docker-compose.dev.yml up -d

# 開発用Docker環境の停止
docker-dev-down:
	docker compose -f docker-compose.dev.yml down -v

# Dockerテストの実行
docker-test:
	./scripts/docker-test.sh

# インタラクティブにNode1に接続
node1:
	docker exec -it p2p-node1 p2p-sync start --port 4001

# インタラクティブにNode2に接続
node2:
	docker exec -it p2p-node2 p2p-sync start --port 4002

# インタラクティブにNode3に接続
node3:
	docker exec -it p2p-node3 p2p-sync start --port 4003

# ログを表示
logs:
	docker compose logs -f

# クリーンアップ
clean:
	cargo clean
	docker compose down -v
	docker compose -f docker-compose.dev.yml down -v