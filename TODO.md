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

## 既知の問題
- [ ] wrangler dev中にsrc変更するとビルドエラーで落ちることがある
