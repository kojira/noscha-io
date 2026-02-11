# メール方針変更タスク

## 概要
- Resendによるメール転送を廃止、webhook必須化
- 購入時(POST /api/order)にwebhook_url必須 → チャレンジ確認 → invoice発行
- メール受信 = R2一時保存 + webhook通知のみ
- RESEND_API_KEYは送信API(POST /api/mail/send)でのみ使用

## 変更内容

### 1. src/types.rs
- OrderRequest: `webhook_url: String` 追加（必須）
- Order: `webhook_url: Option<String>`, `webhook_challenge: Option<String>` 追加
- OrderStatus: `WebhookPending` 追加
- Rental: `webhook_url: Option<String>` 追加 (serde default)
- EmailService: `forward_to` 削除（互換性のためOption<String>+default+skip_serializing_if）
- OrderEmailRequest: `forward_to` をOption化+default（後方互換）
- RenewRequest: `webhook_url: Option<String>` 追加

### 2. src/lib.rs - handle_create_order 変更
新フロー:
1. webhook_urlバリデーション（必須、URLフォーマット）
2. チャレンジトークン生成
3. webhook_urlにPOST: `{"event":"webhook_challenge","challenge_url":"https://{domain}/api/order/{order_id}/confirm/{challenge}"}`
4. Order保存(status=WebhookPending, bolt11は空)
5. レスポンス: `{"order_id","status":"webhook_pending","message":"Check your webhook for the challenge URL"}`
- forward_toバリデーション削除

### 3. src/lib.rs - 新エンドポイント GET /api/order/{order_id}/confirm/{challenge}
1. Order取得、status==WebhookPending確認
2. challenge一致確認
3. invoice作成(coinos)
4. Order更新: status=Pending, bolt11設定
5. レスポンス: `{"order_id","amount_sats","bolt11","expires_at"}`
- ルーティング追加が必要（Router::get("/api/order/:order_id/confirm/:challenge", ...)）

### 4. src/lib.rs - provisioning でwebhook_urlをRentalに保存
mock provisioningとwebhook後のprovisioning両方で `rental.webhook_url = order.webhook_url` 設定

### 5. src/email_shim.js
- Resend転送コード全削除（「No webhook_url - use traditional Resend forwarding」以降）
- webhook_urlがない場合: `message.setReject("No webhook configured")`
- forward_toチェック削除、webhook_urlの存在チェックに変更

### 6. worker-entry.mjs
- handleUpdateSettingsはそのまま残す（webhook_url変更用に使える）

### 7. src/skill.md
- POST /api/order: webhook_url必須パラメータ追加、2段階フロー説明
- GET /api/order/{order_id}/confirm/{challenge} ドキュメント追加
- email forwarding記述をemail receiving(webhook)に変更
- forward_to記述削除

## ビルド & テスト
```bash
# テスト
cargo test

# Rustビルド
wasm-pack build --target bundler --out-dir build/worker --no-typescript

# stagingデプロイ
CLOUDFLARE_API_TOKEN=H5YdXIwlDwTUUW-RzrNGo4sCtkEmAVN3NF75jVDm npx wrangler deploy --env staging
```

## 注意
- 既存rentalデータにwebhook_urlない → serde(default)
- forward_to削除後も既存データにフィールド残る → Option+default
- Rustビルドエラーは修正すること
- cargo testも通すこと
