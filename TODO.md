# noscha.io TODO

## MVP後の改善
- [ ] **メール転送先の確認メール**: Resend API で確認メール送信 → トークンクリックで有効化
  - Resend: https://resend.com/ （月100通無料）
  - noscha.io ドメインにSPF/DKIMレコード追加必要（Cloudflare DNSで設定）
  - 確認完了までメール転送は無効状態にする
- [ ] **送信機能**: 将来的に `alice@noscha.io` からの送信対応（要外部SMTP）
- [ ] **Lightning Address転送**: `alice@noscha.io` → ユーザーのLNアドレスへ転送
- [ ] **ユーザーマイページから設定変更**: 転送先メール変更、サブドメイン設定変更
- [ ] **Tor対応**: .onion アクセス
- [ ] **多言語対応**: 日本語UI
- [ ] **GitHubリポジトリ公開**
- [ ] **Cloudflareデプロイ**: 本番環境へのデプロイ手順

## メール機能改善
- [ ] From表示名に元の送信者名を含める（"Akio Kondo via noscha.io" <noreply@noscha.io>）
- [ ] Reply-Toに元の送信者アドレスを設定
- [ ] メール送信レート制限の実装（1ユーザーあたり/日、/月の上限）
- [ ] Resend無料枠の制限（100通/日、3,000通/月）を考慮した全体制限
- [ ] 制限超過時のユーザー通知
- [ ] 追加メール枠の課金プラン設計（sats建て）
- [ ] 利用規約・注意書きの作成（メール転送の制限事項、禁止事項）
- [ ] スパム対策（大量送信先として悪用されないように）
- [ ] staging環境の整備（noscha-io-staging Worker + staging.noscha.io）

## 既知の問題
- [ ] wrangler dev中にsrc変更するとビルドエラーで落ちることがある
