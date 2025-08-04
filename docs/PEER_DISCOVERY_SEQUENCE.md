# P2Pピア発見とデータ同期シーケンス詳細

## 1. 完全なノード起動シーケンス

```mermaid
sequenceDiagram
    participant User as User
    participant App as P2P-Sync App
    participant Storage as SQLite Storage
    participant Swarm as libp2p Swarm
    participant mDNS as mDNS Service
    participant Network as LAN Network

    User->>App: ./p2p-sync start --port 4002
    
    Note over App: 初期化フェーズ
    App->>Storage: データディレクトリ作成
    App->>Storage: SQLite DB初期化
    App->>Storage: テーブル作成 (kv_store)
    
    App->>Swarm: SwarmBuilder::new()
    App->>Swarm: TCP Transport追加 (port 4002)
    App->>Swarm: QUIC Transport追加 (port 4002)
    App->>Swarm: Noise暗号化設定
    
    Note over App: プロトコル初期化
    App->>Swarm: Gossipsub初期化
    Swarm->>Swarm: トピック"p2p-sync"購読
    App->>Swarm: mDNS初期化
    App->>Swarm: Kademlia DHT初期化
    App->>Swarm: Identify初期化
    
    Note over App: ネットワーク開始
    App->>Swarm: listen_on(TCP:4002)
    App->>Swarm: listen_on(QUIC:4002)
    Swarm->>Network: ポート4002でリスニング開始
    
    App->>mDNS: Service Registration
    mDNS->>Network: Multicast Announcement<br/>(_p2p-sync._tcp.local)
    
    App->>User: === P2P Sync System Started ===<br/>Commands: add, get, delete, list, status
    
    Note over App: 待機状態
    App->>App: stdin入力待機
    App->>Swarm: swarm events待機
```

## 2. 2ノード間でのピア発見詳細シーケンス

```mermaid
sequenceDiagram
    participant A as Node A<br/>(Linux 192.168.11.34:4002)
    participant Net as LAN Multicast<br/>(224.0.0.251:5353)
    participant B as Node B<br/>(Windows 192.168.11.52:4002)

    Note over A,B: 両ノードが起動完了済み

    rect rgb(240, 248, 255)
        Note over A,B: mDNS Service Advertisement Phase
        
        A->>Net: mDNS Service Register<br/>Service: _p2p-sync._tcp.local<br/>Host: node-a.local<br/>IP: 192.168.11.34<br/>Port: 4002<br/>PeerID: 12D3KooWBa1J...
        
        B->>Net: mDNS Service Register<br/>Service: _p2p-sync._tcp.local<br/>Host: node-b.local<br/>IP: 192.168.11.52<br/>Port: 4002<br/>PeerID: 12D3KooWNWS...
        
        Note over A,B: Periodic mDNS Announcements (every 10s)
        A->>Net: Periodic Announcement
        B->>Net: Periodic Announcement
    end

    rect rgb(248, 255, 248)
        Note over A,B: Peer Discovery Phase
        
        A->>Net: mDNS Query<br/>Query: _p2p-sync._tcp.local PTR?
        Net->>A: mDNS Response<br/>Found: node-b.local (192.168.11.52:4002)<br/>PeerID: 12D3KooWNWS...
        
        B->>Net: mDNS Query<br/>Query: _p2p-sync._tcp.local PTR?
        Net->>B: mDNS Response<br/>Found: node-a.local (192.168.11.34:4002)<br/>PeerID: 12D3KooWBa1J...
    end

    rect rgb(255, 248, 248)
        Note over A,B: Connection Establishment Phase
        
        A->>A: add_explicit_peer(12D3KooWNWS)
        A->>A: kad.add_address(peer, addr)
        
        A->>B: TCP Connect Request<br/>dst: 192.168.11.52:4002<br/>src: 192.168.11.34:random
        B->>A: TCP Accept<br/>Connection Established
        
        Note over A,B: Noise Protocol Handshake (XX Pattern)
        A->>B: Noise Message 1<br/>e (ephemeral public key)
        B->>A: Noise Message 2<br/>e, ee, s (ephemeral + static)
        A->>B: Noise Message 3<br/>s, se (static exchange)
        
        Note over A,B: Encrypted Channel Established ✓
    end

    rect rgb(255, 255, 240)
        Note over A,B: Protocol Negotiation Phase
        
        A->>B: Multistream: /meshsub/1.1.0
        B->>A: Multistream: OK /meshsub/1.1.0
        
        A->>B: Multistream: /ipfs/id/1.0.0
        B->>A: Multistream: OK /ipfs/id/1.0.0
        
        Note over A,B: Identify Protocol Exchange
        A->>B: Identify Request<br/>{peer_id, protocols, addresses, agent_version}
        B->>A: Identify Response<br/>{peer_id, protocols, addresses, agent_version}
    end

    rect rgb(240, 255, 240)
        Note over A,B: Gossipsub Mesh Formation
        
        A->>B: Gossipsub: GRAFT<br/>Topic: p2p-sync
        B->>A: Gossipsub: GRAFT Response
        
        B->>A: Gossipsub: GRAFT<br/>Topic: p2p-sync  
        A->>B: Gossipsub: GRAFT Response
        
        Note over A,B: P2P Mesh Network Established ✓
        
        A->>A: Log: "Connection established with peer: 12D3KooWNWS..."
        B->>B: Log: "Connection established with peer: 12D3KooWBa1J..."
    end
```

## 3. データ同期の詳細シーケンス

```mermaid
sequenceDiagram
    participant UserA as User A
    participant NodeA as Node A<br/>(Linux)
    participant StorageA as Storage A
    participant NodeB as Node B<br/>(Windows)  
    participant StorageB as Storage B
    participant UserB as User B

    Note over UserA,UserB: Nodes are connected via Gossipsub mesh

    rect rgb(240, 248, 255)
        Note over UserA,UserB: User A adds data
        
        UserA->>NodeA: > add hello world
        NodeA->>NodeA: validate_key("hello", max_length)
        NodeA->>NodeA: validate_value("world", max_length)
        NodeA->>NodeA: sanitize_input("hello", "world")
        
        NodeA->>StorageA: storage.put("hello", "world")
        StorageA->>StorageA: INSERT OR REPLACE INTO kv_store<br/>(key, value, timestamp)
        StorageA->>NodeA: ✓ Success
        
        NodeA->>UserA: ✓ Added: hello = world
    end

    rect rgb(248, 255, 248)  
        Note over UserA,UserB: Gossipsub Message Broadcasting
        
        NodeA->>NodeA: Create SyncMessage::Put {<br/>  key: "hello",<br/>  value: "world",<br/>  timestamp: 2025-01-04T08:30:15Z<br/>}
        
        NodeA->>NodeA: serde_json::to_vec(message)
        NodeA->>NodeA: Check message size < 1MB
        
        NodeA->>NodeB: Gossipsub Publish<br/>Topic: p2p-sync<br/>Data: {"Put":{"key":"hello","value":"world","timestamp":"2025-01-04T08:30:15Z"}}
        
        Note over NodeB: Message Validation & Processing
        NodeB->>NodeB: Rate limit check (peer_id)
        NodeB->>NodeB: Message size validation
        NodeB->>NodeB: JSON deserialization
        NodeB->>NodeB: validate_key("hello", 256)
        NodeB->>NodeB: validate_value("world", 64KB)
    end

    rect rgb(255, 248, 248)
        Note over UserA,UserB: Storage Synchronization
        
        NodeB->>StorageB: storage.put_with_timestamp(<br/>  "hello", "world", 2025-01-04T08:30:15Z)
        
        StorageB->>StorageB: SELECT timestamp FROM kv_store<br/>WHERE key = 'hello'
        StorageB->>StorageB: Compare timestamps<br/>(incoming vs existing)
        StorageB->>StorageB: INSERT OR REPLACE<br/>(newer timestamp wins)
        
        StorageB->>NodeB: ✓ Storage updated
        NodeB->>NodeB: Log: "Got message from 12D3KooWBa1J: Put { hello = world }"
    end

    rect rgb(255, 255, 240)
        Note over UserA,UserB: User B verifies sync
        
        UserB->>NodeB: > list
        NodeB->>StorageB: storage.list()
        StorageB->>StorageB: SELECT key, value FROM kv_store<br/>WHERE deleted_at IS NULL
        StorageB->>NodeB: [(hello, world)]
        NodeB->>UserB: Stored items (1):<br/>  hello = world
        
        UserB->>NodeB: > get hello
        NodeB->>StorageB: storage.get("hello")
        StorageB->>NodeB: Some("world")
        NodeB->>UserB: ✓ hello = world
    end

    Note over UserA,UserB: Data Successfully Synchronized ✓
```

## 4. 3ノード以上でのメッシュネットワーク形成

```mermaid
sequenceDiagram
    participant A as Node A
    participant B as Node B  
    participant C as Node C
    participant D as Node D

    Note over A,D: All nodes started and doing mDNS discovery

    rect rgb(240, 248, 255)
        Note over A,D: Peer Discovery Matrix
        
        A->>B: mDNS Discover + Connect
        A->>C: mDNS Discover + Connect  
        A->>D: mDNS Discover + Connect
        
        B->>C: mDNS Discover + Connect
        B->>D: mDNS Discover + Connect
        
        C->>D: mDNS Discover + Connect
    end

    rect rgb(248, 255, 248)
        Note over A,D: Gossipsub Mesh Formation
        
        Note over A,D: Each node maintains gossipsub mesh<br/>Degree = min(connected_peers, mesh_n=6)
        
        A->>A: mesh_peers = [B, C, D] (if <= 6 peers)
        B->>B: mesh_peers = [A, C, D]  
        C->>C: mesh_peers = [A, B, D]
        D->>D: mesh_peers = [A, B, C]
        
        Note over A,D: Heartbeat Messages (every 10s)
        A->>B: HEARTBEAT + GRAFT/PRUNE decisions
        A->>C: HEARTBEAT + GRAFT/PRUNE decisions
        A->>D: HEARTBEAT + GRAFT/PRUNE decisions
    end

    rect rgb(255, 248, 248)
        Note over A,D: Message Propagation Test
        
        A->>A: User: add test propagation
        
        Note over A,D: Flood Publishing to Mesh Peers
        A->>B: Gossipsub: {"Put": {"key": "test", ...}}
        A->>C: Gossipsub: {"Put": {"key": "test", ...}}  
        A->>D: Gossipsub: {"Put": {"key": "test", ...}}
        
        Note over A,D: Message Deduplication
        B->>B: Process + Store (message_id tracking)
        C->>C: Process + Store (message_id tracking)
        D->>D: Process + Store (message_id tracking)
        
        Note over A,D: No forwarding needed (full mesh, degree=3)
    end

    rect rgb(255, 255, 240)
        Note over A,D: Mesh Maintenance Example
        
        Note over A,D: If network grows to 10+ nodes
        
        A->>A: mesh_peers.len() > mesh_n (6)<br/>→ Select best 6 peers for mesh<br/>→ PRUNE excess peers  
        
        A->>B: GRAFT (keep in mesh)
        A->>C: GRAFT (keep in mesh)
        A->>D: PRUNE (remove from mesh)
        
        Note over A,D: D becomes gossip peer instead of mesh peer<br/>A will forward messages to D but not directly mesh
    end
```

## 5. エラーハンドリングとリカバリ

```mermaid
sequenceDiagram
    participant A as Node A
    participant B as Node B
    participant Net as Network

    Note over A,B: Normal operation established

    rect rgb(255, 240, 240)
        Note over A,B: Connection Loss Scenario
        
        A->>B: Gossipsub Message
        B--xA: Connection Lost (network issue)
        
        A->>A: Log: "Connection closed with peer B"
        A->>A: connection_manager.handle_connection_closed(B)
        A->>A: gossipsub.remove_explicit_peer(B)
        
        Note over A: Auto-recovery Process
        
        A->>Net: mDNS Re-query for peers
        Net->>A: Found: Node B (192.168.11.52:4002)
        
        A->>B: Reconnection Attempt
        B->>A: Accept Connection
        
        A->>B: Re-establish Noise + Protocols
        B->>A: Protocol Negotiation OK
        
        A->>B: Gossipsub GRAFT (rejoin mesh)
        B->>A: GRAFT Accept
        
        A->>A: Log: "Connection re-established with peer B"
    end

    rect rgb(255, 255, 240)
        Note over A,B: Message Loss & Eventual Consistency
        
        Note over A,B: While B was disconnected:
        A->>A: User: add lost_message test
        A->>A: Storage: lost_message = test (timestamp: T1)
        
        Note over A,B: After B reconnects:
        A->>B: No automatic full sync
        A->>B: Only new messages after reconnection
        
        Note over A,B: Manual sync trigger (future feature):
        A->>B: Sync Request (last_seen_timestamp)
        B->>A: Sync Response (messages after T1)
        
        Note over A,B: Current behavior: 
        Note over A,B: Eventually consistent through user interactions
        Note over A,B: New adds/updates will sync immediately
    end
```

## 6. セキュリティバリデーション詳細

```mermaid
sequenceDiagram
    participant Attacker as Malicious Node
    participant A as Node A  
    participant RateLimit as Rate Limiter
    participant Security as Security Layer

    rect rgb(255, 240, 240)
        Note over Attacker,Security: Attack Mitigation Sequence
        
        Attacker->>A: Rapid Message Flood<br/>(100 messages/second)
        
        A->>RateLimit: rate_limiter.check_rate_limit(peer_id)
        RateLimit->>RateLimit: window_requests[peer_id] > 100/60s?
        RateLimit->>A: RateLimitExceeded Error
        
        A->>A: Log: "Rate limit exceeded for peer"<br/>Drop message
        A->>A: Connection maintained (no ban)
    end

    rect rgb(240, 255, 240)  
        Note over Attacker,Security: Message Size Attack
        
        Attacker->>A: Oversized Message (10MB)
        
        A->>Security: Message size validation
        Security->>Security: message.data.len() > 1MB?
        Security->>A: MessageTooLarge Error
        
        A->>A: Log: "Message too large from peer"<br/>Drop message
    end

    rect
        Note over Attacker,Security: Invalid Data Attack
        
        Attacker->>A: Malformed JSON Message
        
        A->>Security: serde_json::from_slice()
        Security->>A: JSON Parse Error
        
        A->>A: Log: "Invalid message from peer"<br/>Drop message
        
        Attacker->>A: Valid JSON but invalid key/value
        
        A->>Security: validate_key(oversized_key)
        Security->>A: ValidationError
        
        A->>A: Log: "Invalid key from peer"<br/>Drop message
    end

    Note over Attacker,Security: Network remains stable despite attacks
```

## まとめ

このシーケンス図は、P2P Syncが**なぜポート転送なしで動作するか**を明確に示しています：

### 🔑 **キーポイント**

1. **同一LAN内通信** - プライベートIP同士の直接通信
2. **mDNS自動発見** - マルチキャスト（224.0.0.251:5353）による自動ピア発見  
3. **双方向リスニング** - 全ノードがTCP/QUICでリスニング
4. **libp2pの抽象化** - 複雑なネットワーク処理の自動化
5. **堅牢なエラー処理** - 接続断絶からの自動回復

### 📊 **プロトコルスタック**
```
User Commands (add/get/list/status)
        ↓
Application Logic (validation, storage)
        ↓  
Gossipsub (message broadcasting)
        ↓
libp2p (peer management, protocols)
        ↓
Noise (encryption + authentication)  
        ↓
TCP/QUIC (reliable transport)
        ↓
IP (network routing)
```

この詳細なシーケンス図により、P2P Syncの動作原理を完全に理解できます。ポート転送が不要な理由と、分散システムとしての堅牢性を実現する仕組みが明確になります。