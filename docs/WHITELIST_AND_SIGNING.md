# ピアホワイトリストと署名付きデータ構造

## 概要

P2P同期システムに以下のセキュリティ機能を実装しました：

1. **ピアホワイトリスト**: 信頼できるピアのみがデータを同期できるようにする機能
2. **署名付きデータ構造**: すべての同期メッセージがデジタル署名され、送信者の検証が可能
3. **自動鍵配布システム**: ピア間で公開鍵を自動的に発見・交換するメカニズム

## ピアホワイトリスト機能

### ホワイトリストの管理

ホワイトリストはSQLiteデータベースに保存され、以下のコマンドで管理できます：

```bash
# ピアをホワイトリストに追加（公開鍵ファイル付き）
p2p-sync whitelist add <peer_id> [-n name] [-e hours] [-k public_key_file]

# 例: 24時間有効なピアを公開鍵付きで追加
p2p-sync whitelist add 12D3KooWGn8VAsPHsEo32r9JvS9cmj3WTMKjTotHW5evLmZdC9aT -n "Node1" -e 24 -k node1_public.key

# 既存のピアに公開鍵を追加
p2p-sync whitelist add-key <peer_id> <public_key_file>

# ピアをホワイトリストから削除
p2p-sync whitelist remove <peer_id>

# ホワイトリストの一覧表示（公開鍵の有無も表示）
p2p-sync whitelist list

# 特定のピアがホワイトリストに含まれているか確認
p2p-sync whitelist check <peer_id>
```

### ホワイトリストの動作

- 接続時にピアがホワイトリストに含まれているかチェック
- ホワイトリストに含まれないピアからのメッセージは拒否
- 有効期限を設定可能（期限切れのピアは自動的に無効化）

## 署名付きデータ構造

### 実装内容

すべての同期メッセージ（Put/Delete）は以下の構造で署名されます：

```rust
pub struct SignedData<T> {
    pub data: T,
    pub signature: Vec<u8>,
    pub signer: String,  // Peer ID
}
```

### 署名プロセス

1. 各ノードはEd25519鍵ペアを生成
2. メッセージをシリアライズし、SHA256でハッシュ化
3. 秘密鍵でハッシュに署名
4. 署名とPeer IDをメッセージに添付

### 検証プロセス

1. 受信したメッセージから署名者のPeer IDを取得
2. 署名者がホワイトリストに含まれているか確認
3. 保存されている公開鍵を使用してデジタル署名を検証
4. 公開鍵が保存されていない場合はホワイトリストベースの信頼

## セキュリティ上の利点

1. **認証**: ホワイトリストにより、信頼できるピアのみがネットワークに参加可能
2. **完全性**: デジタル署名により、メッセージの改ざんを検出可能
3. **否認防止**: 各メッセージには送信者のPeer IDが含まれる
4. **柔軟性**: 有効期限付きのホワイトリストにより、一時的なアクセス許可が可能

## 使用例

### 1. ノードの起動とPeer ID確認

```bash
p2p-sync start
# 出力例:
# === P2P Sync System Started ===
# Local Peer ID: 12D3KooWGn8VAsPHsEo32r9JvS9cmj3WTMKjTotHW5evLmZdC9aT
```

### 2. 相互にホワイトリストに追加

ノードA:
```bash
p2p-sync whitelist add <ノードBのPeer ID> -n "NodeB"
```

ノードB:
```bash
p2p-sync whitelist add <ノードAのPeer ID> -n "NodeA"
```

### 3. 公開鍵の自動配布

ノード起動後、自動的に公開鍵を配布できます：

```bash
# 自分の公開鍵を通知
> announce-key
✓ Announced public key to all peers

# 欠落している公開鍵を要求
> request-keys
✓ Requested 2 missing public key(s)
```

### 4. データの同期

ホワイトリストに追加されたノード間でのみデータが同期されます：

```bash
> add mykey myvalue
✓ Added: mykey = myvalue
```

## 公開鍵管理

### 公開鍵の保存形式

公開鍵ファイルは以下の形式をサポートします：
- RAW protobuf バイナリ
- Base64エンコードされたprotobuf
- 16進数文字列形式のprotobuf

### 署名検証レベル

1. **完全検証**: 公開鍵が保存されているピアからのメッセージは署名が完全に検証される
2. **信頼ベース**: 公開鍵が未保存のピアは、ホワイトリストに基づく信頼のみ

## 自動鍵配布システム

### 概要

自動鍵配布システムにより、ピア間で公開鍵を自動的に発見・交換することが可能になりました。これにより、手動での公開鍵管理の手間を大幅に削減できます。

### 鍵配布メッセージ

システムは以下の4種類のメッセージをサポートします：

1. **KeyRequest**: 特定のピアの公開鍵を要求
2. **KeyResponse**: 要求された公開鍵を返答
3. **KeyAnnouncement**: 自分の公開鍵をネットワークに通知
4. **WhitelistRequest**: ホワイトリストへの追加を要求

### インタラクティブコマンド

ノード実行中に以下のコマンドが使用できます：

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

### セキュリティ機能

1. **メッセージ有効期限**: デフォルトで24時間の有効期限
2. **リプレイ攻撃防止**: 同じメッセージの重複処理を防止
3. **署名検証**: すべての鍵配布メッセージもデジタル署名による検証
4. **ホワイトリスト認証**: 鍵交換はホワイトリストに含まれるピア間のみ

### 自動化機能

- **自動鍵要求**: ホワイトリストにあるピアで公開鍵が未保存の場合、自動的に要求
- **自動応答**: 他のピアからの鍵要求に自動的に応答
- **鍵検証**: 受信した公開鍵がPeer IDと一致するか自動検証

### 設定オプション

```rust
pub struct KeyDistributionConfig {
    pub auto_share_keys: bool,           // 自動鍵共有
    pub auto_request_keys: bool,         // 自動鍵要求
    pub accept_whitelist_requests: bool, // ホワイトリスト要求の受け入れ
    pub max_message_age_hours: u64,      // メッセージ有効期限（時間）
}
```

## シンプル信頼チェーンシステム (NEW!)

### 概要

シンプルな信頼推薦システムが実装されました。複雑なスコア計算ではなく、1段階の推薦による信頼関係を構築します。

### 信頼推薦メッセージ

```rust
TrustRecommendation {
    recommender: String,    // 推薦者のPeer ID
    recommended: String,    // 推薦されるPeer ID
    name: Option<String>,   // オプション名
    timestamp: DateTime<Utc>,
}
```

### 拡張されたホワイトリストエントリ

```rust
pub struct WhitelistEntry {
    // 既存フィールド
    pub peer_id: String,
    pub name: Option<String>,
    pub public_key: Option<Vec<u8>>,
    pub added_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
    
    // 新しい信頼関係フィールド
    pub recommended_by: Vec<String>,    // 推薦者のリスト
    pub recommendation_count: u32,      // 推薦数
}
```

### 信頼チェーン検証

- **`is_trusted_by_chain()`**: 直接ホワイトリスト + 1段階推薦をチェック
- **`add_recommendation()`**: 信頼推薦を追加
- **自動検証**: 推薦者がホワイトリストに含まれているかを確認

### 新しいCLIコマンド

```bash
# 信頼管理
> recommend-peer <peer_id>
Enter optional name for this peer: MyTrustedNode
✓ Recommended peer 12D3Koo...ABC to the network

# メンテナンス
> cleanup                    # 古い鍵配布データをクリーンアップ
> reload-cache              # ホワイトリストキャッシュを再読み込み
> verify-signature          # 署名検証機能の情報
> test-access-control       # アクセス制御のテスト
```

### セキュリティ機能

1. **推薦者検証**: ホワイトリストに含まれるピアのみが推薦可能
2. **自己推薦防止**: 自分自身を推薦することは不可
3. **送信者検証**: 推薦メッセージの送信者と推薦者が同一であることを確認
4. **重複防止**: 同じピアからの重複推薦を防止

### 動作フロー

```
1. ピアAがホワイトリストに含まれている
2. ピアAが`recommend-peer <PeerB_ID>`を実行
3. ピアBはピアAの推薦により一時的に信頼される
4. ピアBからのメッセージが受け入れられる
5. 管理者が必要に応じてピアBを正式にホワイトリストに追加
```

## 今後の改善点

1. **鍵の回転**: 定期的な鍵の更新メカニズム
2. **ホワイトリストの同期**: ノード間でホワイトリストを安全に共有
3. **監査ログ**: すべてのアクセスと拒否の記録
4. **マルチレベル信頼**: より深い信頼チェーンのサポート