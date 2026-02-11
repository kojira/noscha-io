# noscha.io メール機能仕様書

## ユーザー視点の仕様

### メール受信
1. ユーザーがオーダー時に `webhook_url` を指定（必須）
2. `username@noscha.io` 宛にメールが届く
3. Cloudflare Email Routing (catch-all) → Worker (email_shim.js) で受信
4. R2に保存: `inbox/{username}/{mail_id}.json`
5. `webhook_url` にJSON通知:
   ```json
   {
     "event": "email_received",
     "username": "alice",
     "mail_id": "m_xxxx",
     "from": "sender@example.com",
     "subject": "Hello",
     "date": "2026-02-12T04:00:00Z",
     "view_url": "https://noscha.io/api/mail/alice/m_xxxx"
   }
   ```
6. 有効期限切れユーザー宛メールはreject
7. 受信メールは **1時間で自動削除**（cronで15分毎チェック）

### メール閲覧
- 一覧: `GET /api/mail/{username}?token=mgmt_xxx`
- 個別: `GET /api/mail/{username}/{mail_id}?token=mgmt_xxx`
- 閲覧URL（webhook通知内のview_url）はtokenなしでもアクセス可能（URLが秘密鍵代わり）

### メール送信
- `POST /api/mail/{username}/send`
  ```json
  {
    "to": "recipient@example.com",
    "subject": "Hello",
    "text": "Body text",
    "management_token": "mgmt_xxx"
  }
  ```
- 送信元: `username@noscha.io`（Resend API経由）
- レート制限: **5通/24h per user**

### 支払い完了 (payment_completed)
- 支払い検知・プロビジョニング完了時に `webhook_url` へ POST
- ペイロード: `event`, `order_id`, `username`, `management_token`, `my_page_url`, `expires_at`, `plan`, `amount_sats`, `is_renewal`, `services`

### 非対応（廃止済み）
- `forward_to`（メール転送）: 2026-02-12に廃止。Resend APIの送信枠を消費するため

## 内部アーキテクチャ

### メール受信フロー
```
外部メール → MX → Cloudflare Email Routing (catch-all)
  → Worker email event (worker-entry.mjs → email_shim.js)
  → R2ルックアップ: rentals/{username}.json
  → webhook_url取得 → R2にメール保存 → webhook POST
```

### ファイル構成
- `worker-entry.mjs`: 統合エントリポイント、UnifiedWorkerクラスのemail()メソッド
- `src/email_shim.js`: メール解析（MIME, multipart, charset, encoding）
- `src/lib.rs`: Rust側API（mail list/get/send エンドポイント）
- R2パス:
  - `rentals/{username}.json` — ユーザーデータ（webhook_url含む）
  - `inbox/{username}/{mail_id}.json` — 受信メール
  - `send_emails/{username}.json` — 送信レート制限カウンター

### Cloudflare設定（Dashboard側）
- Email Routing > Catch-all > ワーカーに送信 > noscha-io（アクティブ）
- wrangler.tomlには email trigger 不要（Dashboard設定で動作）

### Resend API
- APIキー: Workers secret `RESEND_API_KEY`
- 無料枠: 100通/日、3,000通/月（グローバル）
- noscha.ioドメインをResendに登録済み
