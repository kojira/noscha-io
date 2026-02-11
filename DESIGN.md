# noscha.io 設計書

## 1. サービス概要

**noscha.io** は、Lightning Network で即時決済して利用できる使い捨てメールアドレス・サブドメイン・NIP-05認証サービスである。

### コンセプト

- **KYCなし・匿名**: アカウント登録不要。Lightning 決済のみで利用開始
- **使い捨て**: 1日〜1年の短期レンタル。期限切れで自動削除
- **即時**: 決済確認後、数秒でメール転送・サブドメイン・NIP-05 が有効化
- **安価**: 1日10sats〜。マイクロペイメントで気軽に使える

### 技術スタック

| コンポーネント | 技術 |
|---|---|
| アプリケーション | Cloudflare Workers |
| ストレージ | Cloudflare R2 (メタデータ JSON) |
| メール | Cloudflare Email Routing API |
| DNS | Cloudflare DNS API |
| 決済 | Coinos API (Lightning Network) |
| フロントエンド | Workers 上の静的HTML (軽量SPA) |

### 提供機能

1. **メール転送**: `{username}@noscha.io` 宛のメールを指定アドレスに転送
2. **サブドメイン**: `{username}.noscha.io` にCNAME/Aレコードを設定
3. **NIP-05認証**: `{username}@noscha.io` で Nostr NIP-05 認証を提供

---

## 2. ユーザーフロー

### 2.1 購入フロー

```
ユーザー                    noscha.io Workers              Coinos API
  |                              |                            |
  |  1. POST /api/order          |                            |
  |  (username, plan, options)   |                            |
  |----------------------------->|                            |
  |                              |  2. POST /api/v2/invoice   |
  |                              |  (amount, webhook)         |
  |                              |--------------------------->|
  |                              |                            |
  |                              |  3. Invoice (bolt11)       |
  |                              |<---------------------------|
  |  4. Invoice返却              |                            |
  |  (order_id, bolt11, qr)      |                            |
  |<-----------------------------|                            |
  |                              |                            |
  |  5. Lightning支払い実行      |                            |
  |  (Wallet -> Coinos)          |                            |
  |------------------------------------------------------------>
  |                              |                            |
  |                              |  6. Webhook: 支払い完了     |
  |                              |<---------------------------|
  |                              |                            |
  |                              |  7. プロビジョニング実行    |
  |                              |  - Email Routing 設定       |
  |                              |  - DNS レコード作成         |
  |                              |  - R2 にデータ保存          |
  |                              |  - NIP-05 キャッシュ更新    |
  |                              |                            |
  |  8. 完了通知                  |                            |
  |  (SSE or ポーリング)          |                            |
  |<-----------------------------|                            |
```

### 2.2 利用フロー

- **メール転送**: 外部から `alice@noscha.io` にメール送信 → Cloudflare Email Routing が Workers を呼び出し → Workers が転送先を R2 から取得 → 転送実行
- **サブドメイン**: `alice.noscha.io` にアクセス → DNS レコード (CNAME/A) で設定先に解決
- **NIP-05**: Nostr クライアントが `https://noscha.io/.well-known/nostr.json?name=alice` にリクエスト → Workers が R2 から pubkey を取得して応答

### 2.3 期限切れフロー

```
Cron Trigger (毎時)
  |
  |  1. R2 から全アクティブレンタルをスキャン
  |  2. expires_at < now のレコードを抽出
  |  3. 各レコードについて:
  |     - Email Routing ルール削除
  |     - DNS レコード削除
  |     - R2 データを status: "expired" に更新
  |     - NIP-05 キャッシュから除去
```

### 2.4 延長フロー

- 期限切れ前に `POST /api/renew` で同一 username の延長が可能
- 新規購入と同様の決済フロー。決済完了後 `expires_at` を延長

---

## 3. API 設計

### 3.1 公開 API

| メソッド | パス | 説明 |
|---|---|---|
| `GET` | `/api/check/{username}` | username の利用可否を確認 |
| `POST` | `/api/order` | 新規注文を作成し Invoice を返却 |
| `GET` | `/api/order/{order_id}/status` | 注文ステータス確認 (SSE対応) |
| `POST` | `/api/renew` | 既存レンタルの延長注文 |
| `GET` | `/api/rental/{username}` | レンタル情報参照 (管理トークン必須) |
| `GET` | `/.well-known/nostr.json` | NIP-05 認証エンドポイント |

### 3.2 内部 API (Webhook)

| メソッド | パス | 説明 |
|---|---|---|
| `POST` | `/api/webhook/coinos` | Coinos 決済完了 Webhook |

### 3.3 エンドポイント詳細

#### `GET /api/check/{username}`

**レスポンス:**
```json
{
  "available": true,
  "username": "alice"
}
```

ユーザー名のバリデーションルール:
- 3〜20文字
- 英小文字、数字、ハイフンのみ
- ハイフンは先頭・末尾不可
- 予約語リスト (`admin`, `www`, `mail`, `api`, `ns1`, `ns2`, `_dmarc`, `autoconfig` 等) は使用不可

#### `POST /api/order`

**リクエスト:**
```json
{
  "username": "alice",
  "plan": "30d",
  "services": {
    "email": {
      "forward_to": "realaddress@example.com"
    },
    "subdomain": {
      "type": "CNAME",
      "target": "mysite.example.com"
    },
    "nip05": {
      "pubkey": "npub1..."
    }
  }
}
```

**レスポンス:**
```json
{
  "order_id": "ord_xxxxxxxx",
  "amount_sats": 100,
  "bolt11": "lnbc...",
  "expires_at": "2025-01-15T12:00:00Z",
  "management_token": "mgmt_xxxxxxxx"
}
```

`management_token` は注文時に一度だけ返却される。レンタル情報の参照・延長に使用。

#### `POST /api/renew`

**リクエスト:**
```json
{
  "username": "alice",
  "management_token": "mgmt_xxxxxxxx",
  "plan": "30d"
}
```

#### `GET /api/order/{order_id}/status`

SSE (Server-Sent Events) 対応。ポーリングも可。

```json
{
  "order_id": "ord_xxxxxxxx",
  "status": "paid",
  "provisioning": "completed",
  "rental": {
    "username": "alice",
    "expires_at": "2025-02-14T12:00:00Z",
    "services": ["email", "subdomain", "nip05"]
  }
}
```

ステータス遷移: `pending` → `paid` → `provisioning` → `completed` / `failed`

#### `GET /.well-known/nostr.json?name={username}`

**レスポンス (NIP-05):**
```json
{
  "names": {
    "alice": "hex_pubkey_here"
  },
  "relays": {
    "hex_pubkey_here": ["wss://relay.example.com"]
  }
}
```

---

## 4. データモデル (R2)

R2 はオブジェクトストレージのため、キー設計が重要。以下のプレフィックスベースの構造を採用する。

### 4.1 キー構造

```
rentals/{username}.json          -- アクティブレンタル情報
orders/{order_id}.json           -- 注文情報
indices/by-expiry/{ISO日付}/{username}  -- 有効期限インデックス (空ボディ)
cache/nostr.json                 -- NIP-05 全ユーザーキャッシュ
```

### 4.2 レンタルオブジェクト

**キー:** `rentals/{username}.json`

```json
{
  "username": "alice",
  "status": "active",
  "management_token_hash": "sha256:xxxxxxxx",
  "created_at": "2025-01-15T12:00:00Z",
  "expires_at": "2025-02-14T12:00:00Z",
  "plan": "30d",
  "services": {
    "email": {
      "enabled": true,
      "forward_to": "realaddress@example.com",
      "cf_rule_id": "rule_xxxxxxxx"
    },
    "subdomain": {
      "enabled": true,
      "type": "CNAME",
      "target": "mysite.example.com",
      "cf_record_id": "rec_xxxxxxxx"
    },
    "nip05": {
      "enabled": true,
      "pubkey_hex": "abcdef1234567890...",
      "relays": ["wss://relay.damus.io"]
    }
  },
  "payment_history": [
    {
      "order_id": "ord_xxxxxxxx",
      "amount_sats": 100,
      "paid_at": "2025-01-15T12:00:05Z"
    }
  ]
}
```

### 4.3 注文オブジェクト

**キー:** `orders/{order_id}.json`

```json
{
  "order_id": "ord_xxxxxxxx",
  "username": "alice",
  "type": "new",
  "plan": "30d",
  "amount_sats": 100,
  "status": "completed",
  "bolt11": "lnbc...",
  "coinos_invoice_id": "inv_xxx",
  "created_at": "2025-01-15T12:00:00Z",
  "paid_at": "2025-01-15T12:00:05Z",
  "provisioned_at": "2025-01-15T12:00:06Z",
  "services_requested": {
    "email": { "forward_to": "realaddress@example.com" },
    "subdomain": { "type": "CNAME", "target": "mysite.example.com" },
    "nip05": { "pubkey": "npub1..." }
  }
}
```

### 4.4 有効期限インデックス

Cron による期限切れ処理を効率化するため、日付別のインデックスを作成する。

**キー:** `indices/by-expiry/2025-02-14/alice`
**ボディ:** 空 (キーの存在だけで十分)

Cron は当日以前の日付プレフィックスを `list()` し、該当レンタルを処理する。

### 4.5 NIP-05 キャッシュ

**キー:** `cache/nostr.json`

全アクティブユーザーの NIP-05 情報を結合したキャッシュ。`/.well-known/nostr.json` へのリクエスト時に R2 から1回の読み取りで応答できる。

```json
{
  "names": {
    "alice": "abcdef1234567890...",
    "bob": "1234567890abcdef..."
  },
  "relays": {
    "abcdef1234567890...": ["wss://relay.damus.io"],
    "1234567890abcdef...": ["wss://relay.nostr.band"]
  }
}
```

プロビジョニング・期限切れ処理時に差分更新する。

---

## 5. Cloudflare Workers 構成

### 5.1 Worker 一覧

単一の Worker で全機能を提供する (モノリス構成)。ルーティングは `itty-router` 等で実装。

```
noscha-worker
├── routes/
│   ├── api.ts          -- /api/* ハンドラ
│   ├── wellknown.ts    -- /.well-known/* ハンドラ
│   └── static.ts       -- / フロントエンド配信
├── services/
│   ├── coinos.ts       -- Coinos API クライアント
│   ├── email.ts        -- Cloudflare Email Routing API
│   ├── dns.ts          -- Cloudflare DNS API
│   ├── provisioner.ts  -- プロビジョニングオーケストレーター
│   └── cleanup.ts      -- 期限切れ処理
├── models/
│   ├── rental.ts       -- レンタルモデル
│   └── order.ts        -- 注文モデル
├── email-worker.ts     -- Email Worker (メール受信ハンドラ)
└── index.ts            -- エントリポイント
```

### 5.2 Bindings

```toml
# wrangler.toml
name = "noscha-worker"
compatibility_date = "2024-01-01"

[[r2_buckets]]
binding = "BUCKET"
bucket_name = "noscha-data"

[vars]
COINOS_API_URL = "https://coinos.io/api"
CF_ZONE_ID = "xxxxxxxx"
DOMAIN = "noscha.io"

# Secrets (wrangler secret put で設定)
# COINOS_API_TOKEN
# CF_API_TOKEN
# WEBHOOK_SECRET
# MANAGEMENT_TOKEN_SALT
```

### 5.3 Routes

```
noscha.io/*              → noscha-worker (HTTP)
noscha.io (Email)        → noscha-worker (Email Worker)
```

### 5.4 Cron Triggers

```toml
[triggers]
crons = ["0 * * * *"]   # 毎時0分に実行
```

Cron ハンドラの処理:
1. `indices/by-expiry/` から期限切れエントリを検索
2. 該当レンタルのクリーンアップ実行
3. R2 ステータス更新、インデックス削除

### 5.5 Email Worker

Cloudflare Email Routing の Email Worker として設定。受信メールを処理する。

```typescript
export default {
  async email(message: EmailMessage, env: Env) {
    // 1. To アドレスから username を抽出
    // 2. R2 から rental を取得
    // 3. status === "active" かつ email.enabled を確認
    // 4. forward_to にメール転送
    // 5. 該当なしの場合は reject
  }
}
```

---

## 6. Coinos API 連携

### 6.1 Coinos API 概要

Coinos (coinos.io) は Lightning Network の決済プラットフォーム。API を通じて Invoice の発行と支払い確認ができる。

### 6.2 認証

```
Authorization: Bearer {COINOS_API_TOKEN}
```

API トークンは Coinos ダッシュボードから取得し、Workers の Secret に保存。

### 6.3 Invoice 発行

**エンドポイント:** `POST https://coinos.io/api/invoice`

```json
{
  "invoice": {
    "amount": 100,
    "type": "lightning",
    "webhook": "https://noscha.io/api/webhook/coinos",
    "secret": "order_secret_xxx"
  }
}
```

**レスポンス:**
```json
{
  "id": "invoice_id",
  "amount": 100,
  "text": "lnbc1u1p...",
  "hash": "payment_hash_xxx"
}
```

### 6.4 支払い確認

2つの方式を併用する:

#### Webhook (プライマリ)

Coinos が支払い完了時に `webhook` URL に POST する。

```json
{
  "id": "invoice_id",
  "amount": 100,
  "hash": "payment_hash_xxx",
  "confirmed": true
}
```

Webhook の検証:
- `secret` フィールドと注文時に設定した `secret` を照合
- リクエスト元の IP を確認 (可能な場合)

#### ポーリング (フォールバック)

Webhook が到達しない場合のフォールバック。フロントエンドからの `GET /api/order/{order_id}/status` リクエスト時に、未確認の注文があれば Coinos API に問い合わせる。

**エンドポイント:** `GET https://coinos.io/api/invoice/{invoice_id}`

### 6.5 Invoice 有効期限

Coinos Invoice のデフォルト有効期限に加え、noscha.io 側でも15分の有効期限を設定。期限切れの注文は `expired` ステータスに更新し、username のロックを解放する。

### 6.6 金額計算

```typescript
function calculateAmount(plan: string, services: string[]): number {
  const basePrices: Record<string, number> = {
    "1d": 10,
    "7d": 50,
    "30d": 100,
    "90d": 250,
    "180d": 450,
    "365d": 800,
  };
  // 全サービスバンドル価格 (email + subdomain + nip05)
  return basePrices[plan];
}
```

---

## 7. Cloudflare Email Routing API 連携

### 7.1 概要

Cloudflare Email Routing を使用して、`{username}@noscha.io` 宛のメールを転送する。2つの方式がある:

- **Routing Rules (catch-all + Email Worker)**: 全メールを Email Worker で受信し、プログラムで転送先を判定
- **Destination Addresses API**: 個別の転送ルールを API で管理

**採用方式: Email Worker (catch-all)**

理由: Routing Rules API は転送先メールアドレスの事前検証 (確認メール送信) が必要であり、匿名サービスの性質と合わない。Email Worker なら `message.forward()` で任意のアドレスに転送可能。

### 7.2 Email Worker での転送処理

```typescript
async email(message: EmailMessage, env: Env) {
  const recipient = message.to;  // alice@noscha.io
  const username = recipient.split("@")[0].toLowerCase();

  // R2 からレンタル情報を取得
  const rental = await getRental(env.BUCKET, username);

  if (!rental || rental.status !== "active" || !rental.services.email?.enabled) {
    message.setReject("Address not found");
    return;
  }

  // 転送実行
  await message.forward(rental.services.email.forward_to);
}
```

### 7.3 設定手順

1. Cloudflare Dashboard で `noscha.io` の Email Routing を有効化
2. Catch-all ルールを Email Worker に設定
3. DNS に MX レコードが自動追加される

### 7.4 制限事項

- Cloudflare Email Routing は受信のみ。送信はサポートしない
- 添付ファイルサイズ上限: 25MB (Cloudflare の制限)
- `message.forward()` は1回のみ呼び出し可能 (複数転送先は非対応)

---

## 8. Cloudflare DNS API 連携

### 8.1 概要

サブドメイン機能として `{username}.noscha.io` の DNS レコードを API で動的に管理する。

### 8.2 API 認証

```
Authorization: Bearer {CF_API_TOKEN}
```

必要な権限: `Zone.DNS:Edit` (対象ゾーン: noscha.io)

### 8.3 レコード作成

**エンドポイント:** `POST https://api.cloudflare.com/client/v4/zones/{zone_id}/dns_records`

```json
{
  "type": "CNAME",
  "name": "alice.noscha.io",
  "content": "mysite.example.com",
  "ttl": 300,
  "proxied": false,
  "comment": "noscha rental: alice, expires: 2025-02-14"
}
```

サポートするレコードタイプ:
- `CNAME`: 他ドメインへのエイリアス
- `A`: IPv4 アドレス
- `AAAA`: IPv6 アドレス

### 8.4 レコード削除

**エンドポイント:** `DELETE https://api.cloudflare.com/client/v4/zones/{zone_id}/dns_records/{record_id}`

期限切れ処理時に呼び出す。`cf_record_id` はレンタルオブジェクトに保存済み。

### 8.5 レコード更新

延長時にレコード内容を変更する場合:

**エンドポイント:** `PATCH https://api.cloudflare.com/client/v4/zones/{zone_id}/dns_records/{record_id}`

### 8.6 Proxy 設定

- デフォルト: `proxied: false` (DNS only)
- ユーザーが HTTPS を必要とする場合に `proxied: true` を選択可能 (Cloudflare Proxy 経由)
- Proxy 有効時は Cloudflare の SSL が自動適用される

### 8.7 制限事項

- Cloudflare Free プランの DNS レコード数上限: 1000件/ゾーン
- 大規模運用時は上位プランまたは複数ゾーンへの分散が必要
- DNS 伝播に最大5分 (TTL 300秒)

---

## 9. NIP-05 実装

### 9.1 NIP-05 仕様

NIP-05 は Nostr Identity Document の仕様。`name@domain` 形式の識別子を Nostr 公開鍵に紐づける。

クライアントは以下の URL にアクセスして検証する:
```
GET https://{domain}/.well-known/nostr.json?name={name}
```

### 9.2 レスポンス形式

```json
{
  "names": {
    "alice": "abcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890"
  },
  "relays": {
    "abcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890": [
      "wss://relay.damus.io",
      "wss://relay.nostr.band"
    ]
  }
}
```

### 9.3 実装方式

```typescript
// /.well-known/nostr.json ハンドラ
async function handleNip05(request: Request, env: Env): Promise<Response> {
  const url = new URL(request.url);
  const name = url.searchParams.get("name")?.toLowerCase();

  if (!name) {
    // 全ユーザーを返す (キャッシュから)
    const cache = await env.BUCKET.get("cache/nostr.json");
    if (!cache) return new Response("{}", { headers: corsHeaders });
    return new Response(cache.body, {
      headers: {
        ...corsHeaders,
        "Content-Type": "application/json",
        "Cache-Control": "max-age=300",
        "Access-Control-Allow-Origin": "*"
      }
    });
  }

  // 個別ユーザー取得
  const rental = await env.BUCKET.get(`rentals/${name}.json`);
  if (!rental) return new Response('{"names":{},"relays":{}}', { headers: corsHeaders });

  const data = await rental.json();
  if (data.status !== "active" || !data.services.nip05?.enabled) {
    return new Response('{"names":{},"relays":{}}', { headers: corsHeaders });
  }

  const response = {
    names: { [name]: data.services.nip05.pubkey_hex },
    relays: { [data.services.nip05.pubkey_hex]: data.services.nip05.relays || [] }
  };

  return new Response(JSON.stringify(response), {
    headers: {
      "Content-Type": "application/json",
      "Access-Control-Allow-Origin": "*",
      "Cache-Control": "max-age=300"
    }
  });
}
```

### 9.4 CORS ヘッダー

NIP-05 仕様では CORS が必須:
```
Access-Control-Allow-Origin: *
```

### 9.5 pubkey 形式の変換

ユーザーは `npub1...` (bech32) 形式で入力するが、NIP-05 レスポンスでは hex 形式が必要。Workers 側で bech32 → hex 変換を行う。

---

## 10. セキュリティ考慮

### 10.1 レート制限

| エンドポイント | 制限 | 実装 |
|---|---|---|
| `POST /api/order` | 10回/分/IP | Cloudflare Rate Limiting Rule |
| `GET /api/check/*` | 30回/分/IP | Cloudflare Rate Limiting Rule |
| `POST /api/webhook/*` | 100回/分 (全体) | Worker 内カウンター |
| `GET /.well-known/nostr.json` | 60回/分/IP | Cloudflare Rate Limiting Rule |

### 10.2 不正利用対策

#### username スクワッティング防止
- 未決済の注文は15分で有効期限切れ → username ロック解放
- 同一 IP からの同時未決済注文数を制限 (最大3件)

#### スパム・フィッシング対策
- 禁止 username リスト (有名ブランド名、紛らわしい名前)
- メール転送は受信のみ (送信不可のため、なりすまし送信は不可)
- 不正利用報告フォーム (`/abuse`) の設置
- 報告されたアカウントの手動レビュー・即時停止機能

#### Webhook セキュリティ
- Coinos Webhook の `secret` フィールドによる検証
- Webhook エンドポイントはべき等 (同一 payment の二重処理防止)

#### management_token セキュリティ
- トークンは SHA-256 ハッシュ化して R2 に保存 (平文保存しない)
- ソルト付きハッシュ (`MANAGEMENT_TOKEN_SALT` を使用)
- トークンは注文完了時に一度だけ返却。再発行不可

#### DNS 悪用防止
- CNAME/A レコードのターゲットを検証 (内部 IP レンジへの設定を禁止)
- `127.0.0.0/8`, `10.0.0.0/8`, `172.16.0.0/12`, `192.168.0.0/16` 等を拒否

### 10.3 データ保護

- R2 のデータは Cloudflare のインフラ上で暗号化 (at-rest)
- 転送先メールアドレスは平文保存 (機能上必要) だが、期限切れ後は削除
- アクセスログは Cloudflare Workers のデフォルトログのみ (最小限)

---

## 11. 利用規約 (ToS) 要点

### 11.1 サービス内容

- noscha.io はメール転送・サブドメイン・NIP-05認証の短期レンタルサービスである
- サービスは「現状有姿 (as-is)」で提供され、可用性の保証はない

### 11.2 禁止事項

- スパムメールの送受信に使用すること
- フィッシング・詐欺行為の助長
- マルウェア配布サイトへのサブドメイン設定
- 児童搾取に関するコンテンツ
- 法律に違反する目的での使用
- 他者の商標・ブランドを騙る username の取得

### 11.3 サービス停止

- 禁止事項に該当する場合、事前通知なくサービスを停止できる
- 停止された場合の返金は行わない

### 11.4 返金ポリシー

- Lightning 決済の性質上、原則として返金不可
- サービス側の障害で正常にプロビジョニングされなかった場合は、同等プランの無料再発行で対応

### 11.5 プライバシー

- 必要最小限のデータのみ保持 (転送先アドレス、DNS ターゲット、Nostr pubkey)
- 期限切れ後、データは削除される (ログは一定期間保持)
- 第三者へのデータ提供は法的要求がない限り行わない
- ユーザー識別情報 (アカウント、メールアドレス等) の登録は不要

### 11.6 免責

- メール転送の遅延・不達について責任を負わない
- DNS 伝播の遅延について責任を負わない
- Cloudflare / Coinos のサービス障害に起因する問題について責任を負わない

---

## 12. 価格テーブル

全サービス (メール転送 + サブドメイン + NIP-05) のバンドル価格。

| プラン | 期間 | 価格 (sats) | 日額換算 |
|---|---|---|---|
| `1d` | 1日 | 10 | 10 sats/日 |
| `7d` | 7日 | 50 | ~7.1 sats/日 |
| `30d` | 30日 | 100 | ~3.3 sats/日 |
| `90d` | 90日 | 250 | ~2.8 sats/日 |
| `180d` | 180日 | 450 | 2.5 sats/日 |
| `365d` | 365日 | 800 | ~2.2 sats/日 |

### 個別サービス価格 (将来の拡張)

初期リリースでは全サービスバンドルのみ。将来的に個別選択を検討:

| サービス | 30日価格 (sats) |
|---|---|
| メール転送のみ | 50 |
| サブドメインのみ | 50 |
| NIP-05のみ | 30 |
| 全サービスバンドル | 100 |

---

## 13. 将来の拡張案

### 13.1 Lightning Address 転送

`alice@noscha.io` を Lightning Address として機能させ、受信した支払いをユーザー指定の Lightning Address に転送する。

**実装方式:**
- `/.well-known/lnurlp/{username}` エンドポイントを追加
- LNURL-pay プロトコルに対応
- ユーザー指定の Lightning Address の LNURL を取得し、プロキシとして応答
- Invoice は転送先の LNURL から取得して返却 (noscha.io は資金を預からない)

### 13.2 Nostr Relay プロキシ

`wss://noscha.io/{username}` として Nostr Relay プロキシを提供。Cloudflare Workers の WebSocket 対応で実装可能。

### 13.3 カスタムドメイン対応

ユーザーが自分のドメインを持ち込み、noscha.io のインフラでメール転送・NIP-05 を利用する。

### 13.4 Wildcard サブドメイン

`*.alice.noscha.io` のようなワイルドカードサブドメインを上位プランとして提供。

### 13.5 API キー発行

開発者向けに API キーを発行し、プログラムからレンタルの管理・一括作成を可能にする。

### 13.6 Tor / I2P 対応

`.onion` アドレスでのアクセスに対応。Cloudflare の Onion Routing 機能を活用。

### 13.7 Web UI の強化

- レンタル管理ダッシュボード (management_token でログイン)
- メール受信履歴の閲覧 (R2 に一時保存)
- DNS レコードの変更 UI

### 13.8 マルチドメイン展開

`noscha.io` 以外のドメインでも同一インフラでサービスを提供。ユーザーが好みのドメインを選択できる。

### 13.9 Zap 統合

NIP-57 (Zaps) に対応。`alice@noscha.io` への Zap を転送先 Lightning Address にルーティングする。

### 13.10 リファラルプログラム

既存ユーザーの紹介で新規ユーザーが割引を受けられるプログラム。Lightning で紹介報酬を即時支払い。
