# P2P Sync ネットワークアーキテクチャ

## 概要

P2P Syncは、中央サーバーを必要としない分散型同期システムです。libp2pを使用して、複数のプロトコルを組み合わせた堅牢なP2Pネットワークを構築しています。

## アーキテクチャ図

```
┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐
│   Node A        │    │   Node B        │    │   Node C        │
│ 192.168.1.100   │    │ 192.168.1.101   │    │ 192.168.1.102   │
│ Port: 4001      │    │ Port: 4002      │    │ Port: 4003      │
└─────────────────┘    └─────────────────┘    └─────────────────┘
         │                       │                       │
         └───────── LAN ──────────┼───────────────────────┘
                    │
              ┌─────────────┐
              │   Router    │
              │ 192.168.1.1 │
              └─────────────┘
```

## プロトコルスタック

```
┌─────────────────────────────────────────┐
│           Application Layer             │
│        (Key-Value Sync Logic)          │
├─────────────────────────────────────────┤
│            Gossipsub                    │
│        (Message Broadcasting)          │
├─────────────────────────────────────────┤
│              libp2p                     │
│   ┌─────────┬─────────┬─────────────┐   │
│   │  mDNS   │   DHT   │  Identify   │   │
│   │(Discover│(Kademlia│ (Protocol   │   │
│   │   y)    │)        │   Info)     │   │
│   └─────────┴─────────┴─────────────┘   │
├─────────────────────────────────────────┤
│         Transport Layer                 │
│     ┌─────────────┬─────────────────┐   │
│     │    TCP      │      QUIC       │   │
│     │             │   (over UDP)    │   │
│     └─────────────┴─────────────────┘   │
├─────────────────────────────────────────┤
│         Security Layer                  │
│              Noise                      │
│        (Encryption + Auth)              │
└─────────────────────────────────────────┘
```

## ノード起動とピア発見のシーケンス

```mermaid
sequenceDiagram
    participant A as Node A<br/>192.168.1.100:4001
    participant Router as Router<br/>192.168.1.1
    participant B as Node B<br/>192.168.1.101:4002
    participant Network as LAN Network<br/>224.0.0.251:5353

    Note over A,B: ノード起動フェーズ
    
    A->>A: 1. P2P Swarm初期化
    A->>A: 2. TCP/QUIC Listener開始
    A->>A: 3. mDNS Service開始
    A->>A: 4. Kademlia DHT初期化
    A->>A: 5. Gossipsub購読開始

    B->>B: 1. P2P Swarm初期化
    B->>B: 2. TCP/QUIC Listener開始
    B->>B: 3. mDNS Service開始
    B->>B: 4. Kademlia DHT初期化
    B->>B: 5. Gossipsub購読開始

    Note over A,B: ピア発見フェーズ
    
    A->>Network: 6. mDNS Advertisement<br/>(_p2p-sync._tcp.local)
    B->>Network: 6. mDNS Advertisement<br/>(_p2p-sync._tcp.local)
    
    Network->>A: 7. mDNS Response<br/>(Node B discovered)
    Network->>B: 7. mDNS Response<br/>(Node A discovered)

    Note over A,B: 接続確立フェーズ
    
    A->>B: 8. TCP/QUIC Connection Request<br/>(/ip4/192.168.1.101/tcp/4002)
    B->>A: 9. Connection Accept
    
    A->>B: 10. Noise Handshake (暗号化確立)
    B->>A: 11. Noise Handshake Response
    
    A->>B: 12. Protocol Negotiation<br/>(/meshsub/1.1.0, /ipfs/id/1.0.0)
    B->>A: 13. Protocol Agreement
    
    A->>B: 14. Identify Protocol<br/>(Peer info exchange)
    B->>A: 15. Identify Response
    
    A->>B: 16. Gossipsub GRAFT<br/>(Topic: p2p-sync)
    B->>A: 17. Gossipsub GRAFT Response

    Note over A,B: データ同期フェーズ
    
    A->>A: 18. User: add key1 value1
    A->>B: 19. Gossipsub Publish<br/>{"Put": {"key": "key1", "value": "value1", "timestamp": "..."}}
    B->>B: 20. Storage Update<br/>(key1 = value1)
    
    B->>B: 21. User: add key2 value2
    B->>A: 22. Gossipsub Publish<br/>{"Put": {"key": "key2", "value": "value2", "timestamp": "..."}}
    A->>A: 23. Storage Update<br/>(key2 = value2)
```

## mDNSによる自動発見の詳細

```mermaid
sequenceDiagram
    participant A as Node A
    participant M as Multicast<br/>224.0.0.251:5353
    participant B as Node B
    participant C as Node C

    Note over A,C: mDNS Service Registration

    A->>M: Register Service<br/>_p2p-sync._tcp.local<br/>192.168.1.100:4001<br/>PeerID: 12D3Koo...ABC
    
    B->>M: Register Service<br/>_p2p-sync._tcp.local<br/>192.168.1.101:4002<br/>PeerID: 12D3Koo...DEF
    
    C->>M: Register Service<br/>_p2p-sync._tcp.local<br/>192.168.1.102:4003<br/>PeerID: 12D3Koo...GHI

    Note over A,C: Service Discovery

    A->>M: Query: _p2p-sync._tcp.local
    M->>A: Response: Node B at 192.168.1.101:4002
    M->>A: Response: Node C at 192.168.1.102:4003
    
    B->>M: Query: _p2p-sync._tcp.local
    M->>B: Response: Node A at 192.168.1.100:4001
    M->>B: Response: Node C at 192.168.1.102:4003
    
    C->>M: Query: _p2p-sync._tcp.local
    M->>C: Response: Node A at 192.168.1.100:4001
    M->>C: Response: Node B at 192.168.1.101:4002

    Note over A,C: Direct P2P Connections Established
    
    A->>B: Direct Connection
    A->>C: Direct Connection
    B->>C: Direct Connection
```

## Gossipsubメッセージ配信

```mermaid
sequenceDiagram
    participant A as Node A
    participant B as Node B
    participant C as Node C
    participant D as Node D

    Note over A,D: メッシュネットワーク構築済み

    A->>A: User: add key1 value1
    A->>A: Local Storage Update
    
    Note over A,D: Gossipsub Flood Publishing
    
    A->>B: Publish: {"Put": {"key": "key1", ...}}
    A->>C: Publish: {"Put": {"key": "key1", ...}}
    
    Note over A,D: Message Propagation
    
    B->>D: Forward: {"Put": {"key": "key1", ...}}
    C->>D: Forward: {"Put": {"key": "key1", ...}}
    
    Note over A,D: Duplicate Detection & Storage
    
    B->>B: Storage Update (key1 = value1)
    C->>C: Storage Update (key1 = value1)
    D->>D: Storage Update (key1 = value1)<br/>+ Duplicate Detection
    
    Note over A,D: Heartbeat & Mesh Maintenance
    
    A->>B: Heartbeat + GRAFT/PRUNE
    B->>C: Heartbeat + GRAFT/PRUNE
    C->>D: Heartbeat + GRAFT/PRUNE
    D->>A: Heartbeat + GRAFT/PRUNE
```

## 接続確立の状態遷移

```mermaid
stateDiagram-v2
    [*] --> Starting: アプリケーション起動
    
    Starting --> Listening: TCP/QUICリスナー開始
    Listening --> Discovering: mDNS開始
    Discovering --> PeerFound: ピア発見
    
    PeerFound --> Connecting: 接続試行
    Connecting --> Handshaking: TCPコネクション確立
    Handshaking --> Authenticating: Noiseハンドシェイク
    Authenticating --> Negotiating: プロトコルネゴシエーション
    Negotiating --> Identifying: Identifyプロトコル
    Identifying --> Subscribing: Gossipsub購読
    Subscribing --> Connected: 接続完了
    
    Connected --> Syncing: データ同期開始
    Syncing --> Connected: 継続的同期
    
    Connected --> Disconnected: 接続切断
    Disconnected --> Discovering: 再発見
    
    Connecting --> Failed: 接続失敗
    Failed --> Discovering: リトライ
```

## ネットワークトポロジーの種類

### 1. 同一LANでの接続

```
┌─────────────────────────────────────────────────────────────┐
│                    LAN (192.168.1.0/24)                    │
│                                                             │
│  ┌──────────┐    ┌──────────┐    ┌──────────┐              │
│  │ Node A   │    │ Node B   │    │ Node C   │              │
│  │.100:4001 │◄──►│.101:4002 │◄──►│.102:4003 │              │
│  └──────────┘    └──────────┘    └──────────┘              │
│                                                             │
└─────────────────────────────────────────────────────────────┘

特徴:
✓ ポート転送不要
✓ mDNS自動発見
✓ 高速・低遅延
✓ ファイアウォール設定不要
```

### 2. インターネット越しの接続

```
┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐
│   LAN A         │    │   Internet      │    │   LAN B         │
│                 │    │                 │    │                 │
│ ┌─────────────┐ │    │                 │    │ ┌─────────────┐ │
│ │   Node A    │ │    │                 │    │ │   Node B    │ │
│ │ 192.168.1.2 │◄┼────┼─────────────────┼────┼►│ 10.0.0.100  │ │
│ └─────────────┘ │    │                 │    │ └─────────────┘ │
│      │          │    │                 │    │      │          │
│ ┌─────────────┐ │    │                 │    │ ┌─────────────┐ │
│ │Router/NAT   │ │    │                 │    │ │Router/NAT   │ │
│ │1.2.3.4:4001 │ │    │                 │    │ │5.6.7.8:4002 │ │
│ └─────────────┘ │    │                 │    │ └─────────────┘ │
└─────────────────┘    └─────────────────┘    └─────────────────┘

必要な設定:
- ポート転送 (4001 → Node A)
- 外部IP指定での接続
- ファイアウォール開放
- 手動ピア指定
```

### 3. ハイブリッド接続

```
┌─────────────────────────────────────────────────────────────┐
│                    LAN (192.168.1.0/24)                    │
│                                                             │
│  ┌──────────┐    ┌──────────┐                              │
│  │ Node A   │◄──►│ Node B   │                              │
│  │.100:4001 │    │.101:4002 │                              │
│  └──────────┘    └──────────┘                              │
│       │               │                                     │
└───────┼───────────────┼─────────────────────────────────────┘
        │               │
        └───────────────┼─────── Internet ──────┐
                        │                       │
                   ┌──────────┐         ┌──────────┐
                   │ Node C   │         │ Node D   │  
                   │External  │◄───────►│External  │
                   │5.6.7.8   │         │9.10.11.12│
                   └──────────┘         └──────────┘

特徴:
- LANノード同士: mDNS自動発見
- 外部ノード: 手動接続 + DHT経由発見
- 混在環境での柔軟な接続
```

## セキュリティモデル

### 1. 暗号化レイヤー

```
Application Data
        ↓
   Gossipsub Layer (メッセージ署名)
        ↓
    Noise Protocol (AES-256-GCM)
        ↓
     TCP/QUIC Transport
        ↓
    Network (IP/Ethernet)
```

### 2. 認証フロー

```mermaid
sequenceDiagram
    participant A as Node A
    participant B as Node B

    Note over A,B: Noise Handshake (XX Pattern)

    A->>B: 1. Noise_XX_25519_ChaChaPoly_BLAKE2s
    A->>B: 2. e (ephemeral public key)
    
    B->>A: 3. e, ee, s (ephemeral + static keys)
    
    A->>B: 4. s, se (static key exchange)
    
    Note over A,B: Encrypted Channel Established
    
    A->>B: 5. Identify Protocol<br/>(Peer ID verification)
    B->>A: 6. Identify Response<br/>(Peer ID + supported protocols)
    
    Note over A,B: Mutual Authentication Complete
```

## パフォーマンス特性

### 接続確立時間

| フェーズ | 典型的時間 | 説明 |
|----------|------------|------|
| mDNS発見 | 100-500ms | ローカルネットワーク内での発見 |
| TCP接続 | 1-10ms | 同一LAN内での接続確立 |
| Noiseハンドシェイク | 5-20ms | 暗号化チャネル確立 |
| プロトコルネゴシエーション | 10-50ms | libp2pプロトコル合意 |
| 総接続時間 | 116-580ms | 完全な接続確立まで |

### メッセージ伝播

| ノード数 | 平均伝播時間 | 最大ホップ数 |
|----------|--------------|--------------|
| 2-5 | < 10ms | 1 |
| 6-20 | 10-50ms | 2-3 |
| 21-100 | 50-200ms | 3-4 |

## トラブルシューティング

### よくある問題と解決策

1. **mDNS発見が動作しない**
   - ファイアウォールでマルチキャスト（UDP 5353）がブロックされている
   - 異なるVLANに配置されている
   - mDNSサービスが無効になっている

2. **接続確立後すぐに切断される**
   - Noiseハンドシェイクの失敗
   - プロトコルバージョンの不一致
   - ネットワーク不安定

3. **メッセージが同期されない**
   - Gossipsubトピックの不一致
   - メッセージサイズ制限超過
   - 暗号化/復号化エラー

### デバッグコマンド

```bash
# 詳細ログでの起動
RUST_LOG=debug ./p2p-sync start --port 4001

# 接続状況確認
> status
> peers
> info

# ネットワーク診断
./debug-network.sh
```

## 将来の拡張

### 計画中の機能

1. **NAT Traversal強化**
   - STUN/TURNサーバー対応
   - UPnPによる自動ポート転送
   - WebRTC接続サポート

2. **DHT機能拡充**
   - コンテンツベースのルーティング
   - 分散ストレージ機能
   - レプリケーション制御

3. **セキュリティ強化**
   - 証明書ベース認証
   - 権限管理システム
   - 監査ログ機能

---

このドキュメントは、P2P Syncのネットワークアーキテクチャの完全な理解を提供します。実装の詳細や設定方法については、各々の技術ドキュメントを参照してください。