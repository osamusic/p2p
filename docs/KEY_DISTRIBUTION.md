# 自動鍵配布システム (Automated Key Distribution)

## 概要 (Overview)

P2P同期システムに実装された自動鍵配布機能により、ピア間で公開鍵を自動的に発見・交換できます。

The automated key distribution system enables automatic discovery and exchange of public keys between peers in the P2P synchronization system.

## 機能 (Features)

### 🔑 鍵配布メッセージ (Key Distribution Messages)

| メッセージタイプ | 説明 | 用途 |
|------------------|------|------|
| `KeyRequest` | 公開鍵の要求 | 特定のピアの公開鍵を取得 |
| `KeyResponse` | 公開鍵の応答 | 要求に対する公開鍵の提供 |
| `KeyAnnouncement` | 公開鍵の通知 | 自分の公開鍵をネットワークに通知 |
| `WhitelistRequest` | ホワイトリスト要求 | 新しいピアがホワイトリスト追加を要求 |

### 🛠️ インタラクティブコマンド (Interactive Commands)

ノード実行中に以下のコマンドが利用可能です：

```bash
# 自分の公開鍵をすべてのピアに通知
> announce-key
✓ Announced public key to all peers

# 欠落している公開鍵を自動的に要求
> request-keys
✓ Requested 3 missing public key(s)

# ホワイトリストへの追加を要求
> request-whitelist
Enter your name (optional): MyNode
✓ Sent whitelist request to all peers
```

## セキュリティ (Security)

### 🔒 セキュリティ機能

1. **メッセージ有効期限**: 24時間のデフォルト有効期限
2. **リプレイ攻撃防止**: 重複メッセージの検出と拒否
3. **署名検証**: Ed25519署名による完全性検証
4. **ホワイトリスト認証**: 承認されたピア間のみの鍵交換

### 🛡️ 検証プロセス

```
1. メッセージ受信 → 2. 有効期限チェック → 3. リプレイ攻撃チェック
       ↓
4. 署名検証 → 5. ホワイトリスト確認 → 6. 公開鍵検証
       ↓
7. 処理実行 → 8. 必要に応じて応答
```

## 設定 (Configuration)

### KeyDistributionConfig

```rust
pub struct KeyDistributionConfig {
    /// 自動鍵共有を有効にする
    pub auto_share_keys: bool,           // デフォルト: true
    
    /// 自動鍵要求を有効にする  
    pub auto_request_keys: bool,         // デフォルト: true
    
    /// ホワイトリスト要求の受け入れ
    pub accept_whitelist_requests: bool, // デフォルト: false
    
    /// メッセージ有効期限（時間）
    pub max_message_age_hours: u64,      // デフォルト: 24
}
```

## 使用例 (Usage Examples)

### シナリオ1: 新しいネットワークの構築

```bash
# ノードA起動
p2p-sync start --port 8000

# ノードB起動  
p2p-sync start --port 8001 --dial /ip4/127.0.0.1/tcp/8000

# ノードAでBをホワイトリストに追加
p2p-sync whitelist add <NodeB_PeerID> -n "NodeB"

# ノードBでAをホワイトリストに追加
p2p-sync whitelist add <NodeA_PeerID> -n "NodeA"

# 各ノードで公開鍵を自動配布
> announce-key
> request-keys
```

### シナリオ2: 既存ネットワークへの参加

```bash
# 新しいノード起動
p2p-sync start --dial /ip4/existing-node/tcp/8000

# 既存ノードにホワイトリスト追加を要求
> request-whitelist
Enter your name (optional): NewNode

# 管理者が手動でホワイトリストに追加後、鍵を要求
> request-keys
```

## トラブルシューティング (Troubleshooting)

### よくある問題

1. **"Message from non-whitelisted peer"**
   - 解決策: 送信者をホワイトリストに追加する
   - コマンド: `p2p-sync whitelist add <peer_id>`

2. **"Invalid signature from peer"**
   - 解決策: 公開鍵を再取得する
   - コマンド: `request-keys`

3. **"Ignoring old key distribution message"**
   - 解決策: 時刻同期を確認する
   - ヒント: メッセージは24時間で期限切れ

### デバッグログ

詳細なログを有効にするには：

```bash
RUST_LOG=info p2p-sync start
```

## アーキテクチャ (Architecture)

### 鍵配布フロー

```
┌─────────────┐    KeyRequest     ┌─────────────┐
│   Peer A    │ ───────────────→  │   Peer B    │
│             │                   │             │
│             │ ←─────────────────  │             │
└─────────────┘    KeyResponse    └─────────────┘
```

### メッセージ構造

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum KeyDistributionMessage {
    KeyRequest {
        requestor: String,      // 要求者のPeer ID
        target: String,         // 要求対象のPeer ID  
        timestamp: DateTime<Utc>,
    },
    KeyResponse {
        target: String,         // 対象のPeer ID
        public_key: Vec<u8>,    // プロトバッファ形式の公開鍵
        timestamp: DateTime<Utc>,
    },
    // ... 他のメッセージタイプ
}
```

## 今後の計画 (Future Plans)

- [x] 信頼チェーンによる相互認証 ✅ **実装完了!**
- [ ] 鍵の定期回転機能
- [ ] ネットワーク全体での鍵配布統計
- [ ] 階層的ホワイトリスト管理

## 🆕 実装済み機能アップデート (v0.2.0)

### **シンプル信頼チェーンシステム**

基本的な信頼推薦システムが実装されました：

```bash
# 新しいコマンド
> recommend-peer 12D3Koo...ABC  # ピアを推薦
> cleanup                        # データクリーンアップ  
> reload-cache                   # キャッシュ再読み込み
```

### **信頼関係データベース**

ホワイトリストが拡張され、推薦情報を保存：

```sql
-- 新しいフィールド
recommended_by TEXT DEFAULT '[]'        -- 推薦者リスト (JSON)
recommendation_count INTEGER DEFAULT 0  -- 推薦数
```