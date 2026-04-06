# rust_web_browser_engineering

Rust でブラウザの主要パイプラインを自前実装している、学習兼ポートフォリオ向けの
Web ブラウザ/ブラウザエンジンプロジェクトです。

`browser.engineering` の考え方をベースにしながら、ページ取得、HTML 解析、
CSS 適用、レイアウト計算、display list 生成、GPU 描画までを一連の流れとして
実装しています。

## ポートフォリオ向け要約

このプロジェクトで伝えたいポイントは次の通りです。

- `http://` / `https://` / `file://` の取得処理を Rust で実装
- HTML から DOM を構築し、文書構造の補完やエンティティ展開を実装
- CSS の一部を解析し、セレクタマッチングと specificity を考慮してスタイル適用
- block / inline ベースのレイアウト計算を実装
- テキスト折り返し、画像の自然サイズ反映、余白や境界線の計算に対応
- `wgpu` を使ってテキスト、背景、角丸 border、画像を GPU 描画
- リンクの hover、クリック遷移、スクロールを実装

## プロジェクト概要

一般的なブラウザが内部で行っている処理を、できるだけ小さな構成で追体験することを
目的にしたプロジェクトです。単なるテキスト表示ではなく、現在は以下のような
一連のブラウザ処理に対応しています。

1. URL を解釈してページを取得する
2. HTML を解析して DOM ツリーを構築する
3. CSS を収集してスタイルツリーを作る
4. レイアウトツリーを構築して座標とサイズを計算する
5. display list に変換する
6. `wgpu` でテキスト、背景、枠線、画像を描画する

## 実装済み機能

### ネットワーク/リソース取得

- `http://`, `https://`, `file://` に対応
- リダイレクト追従に対応
- `chunked`, `gzip`, `br` のレスポンスを処理
- 画像の取得とデコードに対応

### HTML

- 基本的な HTML パース
- `html` / `head` / `body` の補完
- HTML エンティティのデコード
- `script` / `style` の raw text 扱い

### CSS

- `<style>` に対応
- `<link rel="stylesheet">` に対応
- `@import` の再帰展開に対応
- セレクタ対応:
  - タグセレクタ
  - `.class`
  - `#id`
  - `tag.class`
  - 子孫セレクタ
- 継承プロパティの一部に対応

### レイアウト

- block / inline レイアウト
- inline formatting context の簡易実装
- テキスト折り返し
- `margin`, `padding`, `border`, `border-radius`
- 画像の CSS サイズ、HTML 属性サイズ、自然サイズの反映
- `px`, `%`, `vw`, `vh` の長さ解釈

### 描画/操作

- テキスト描画
- 背景色描画
- border 描画
- border-radius 描画
- リスト bullet 描画
- 画像描画
- リンク下線描画
- リンク hover 時の色変化
- クリックによるページ遷移
- マウスホイールによるスクロール

## 技術的な見どころ

- `std::net` と `native-tls` を使い、ブラウザ向けの最小限の HTTP/HTTPS 取得を実装
- CSS の selector matching に specificity を導入
- anonymous block box を使って inline 要素の流し込みを実装
- `fontdue` でグリフを rasterize し、glyph atlas を使ってテキスト描画
- `wgpu` + WGSL により、矩形、border、画像、文字を個別パイプラインで描画
- 画像は自然サイズ取得と GPU テクスチャ化の両方を行う構成

## 使用技術

- Language: Rust (Edition 2024)
- Window/Event: `winit`
- Rendering: `wgpu`, WGSL
- Text rasterization: `fontue`
- TLS: `native-tls`d
- Compression: `flate2`, `brotli`
- Image decoding: `image`

## 現在の制限

学習・実験用の実装のため、以下はまだ未対応です。

- JavaScript 実行
- フォーム送信やアプリケーション的なページ挙動
- アドレスバー、タブ、戻る/進むなどのブラウザ UI
- Cookie、Storage、Cache などの永続化機能
- 実ブラウザ水準の HTML/CSS 互換性
- クロスプラットフォームなフォント読み込み

また、CSS は最小実装のため次のようなものは未対応です。

- 子セレクタ (`>`)
- 隣接兄弟セレクタ (`+`)
- 属性セレクタ (`[attr]`)
- 疑似クラス/疑似要素
- `@media` など多くの at-rule

## 実行方法

URL を第 1 引数として渡して起動します。

### リモートページ

```bash
cargo run -- https://example.com
```

### ローカルファイル

```bash
cargo run -- file:///D:/path/to/page.html
```

起動すると次のタイトルのネイティブウィンドウが開きます。

```text
Rust Browser (winit0.29 + wgpu0.19)
```

## 動作環境メモ

現状は Windows 前提の実装です。

- `src/main.rs` で `C:\Windows\Fonts\meiryo.ttc` を直接読んでいます
- `src/gpu.rs` でも Windows の日本語フォントを候補にしています

そのため、非 Windows 環境で動かすにはフォント読み込み部分の修正が必要です。

## ディレクトリ構成

- `src/main.rs`
  - エントリーポイント
  - ページ構築
  - イベントループ
  - hover / click / scroll 処理
- `src/url.rs`
  - URL 解析
  - 相対 URL 解決
- `src/http.rs`
  - `http` / `https` / `file` の取得
  - リダイレクト処理
  - レスポンスデコード
- `src/dom.rs`
  - DOM ノード定義
- `src/html.rs`
  - HTML パーサ
  - 文書構造の補完
- `src/css.rs`
  - CSS パーサ
  - shorthand 展開
- `src/style.rs`
  - セレクタマッチング
  - specificity
  - style tree 構築
- `src/layout.rs`
  - block / inline レイアウト
  - テキスト折り返し
  - 画像サイズ計算
- `src/display.rs`
  - display list 生成
- `src/gpu.rs`
  - GPU パイプライン
  - glyph atlas
  - 描画処理
- `src/image_loader.rs`
  - 画像取得
  - デコード
  - 自然サイズ取得
- `src/*.wgsl`
  - 矩形、文字、画像描画用シェーダ

## 補足

`src/html/` 配下にはローカル確認用のサンプル HTML を含めています。

このリポジトリは完成品のブラウザというより、ブラウザ内部の仕組みを Rust で
段階的に実装し、その理解と実装力を示すためのプロジェクトです。
