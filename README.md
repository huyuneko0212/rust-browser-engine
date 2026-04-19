# rust_web_browser_engineering

Rust でブラウザの主要パイプラインを自前実装している、学習兼ポートフォリオ向けの
Web ブラウザ/ブラウザエンジンプロジェクトです。

`browser.engineering` の考え方をベースにしながら、ページ取得、HTML 解析、
CSS 適用、レイアウト計算、display list 生成、GPU 描画までを一連の流れとして
実装しています。

現在は URL を 1 つ渡してネイティブウィンドウを開き、ページを表示・スクロール・
リンク遷移できる最小ブラウザとして動きます。

## ポートフォリオ向け要約

このプロジェクトで伝えたいポイントは次の通りです。

- `http://` / `https://` / `file://` の取得処理を Rust で実装
- HTML から DOM を構築し、文書構造の補完やエンティティ展開を実装
- UA CSS、`<style>`、外部 CSS、`@import` を取り込み、簡易 stylesheet を構築
- CSS の一部を解析し、セレクタマッチングと specificity を考慮してスタイル適用
- block / inline ベースのレイアウト計算を実装
- テキスト折り返し、画像の自然サイズ反映、余白や境界線の計算に対応
- `wgpu` を使ってテキスト、背景、角丸 border、画像を GPU 描画
- リンクの hover、クリック遷移、リサイズ時の再レイアウト、スクロールを実装

## プロジェクト概要

一般的なブラウザが内部で行っている処理を、できるだけ小さな構成で追体験することを
目的にしたプロジェクトです。単なるテキスト表示ではなく、現在は以下のような
一連のブラウザ処理に対応しています。

1. URL を解釈してページを取得する
2. HTML を解析して DOM ツリーを構築する
3. UA CSS / 埋め込み CSS / 外部 CSS を収集して stylesheet を作る
4. DOM と CSS から style tree を作る
5. layout tree を構築して座標とサイズを計算する
6. display list に変換する
7. `wgpu` でテキスト、背景、枠線、画像を描画する
8. マウス操作に応じて hover、scroll、navigation、resize relayout を行う

アドレスバーやタブを持つ完成品ブラウザではなく、ブラウザエンジンの中核処理を
小さく動かすアプリです。

## 実装済み機能

### ネットワーク/リソース取得

- `http://`, `https://`, `file://` に対応
- HTTP/HTTPS は `std::net::TcpStream` と `native-tls` で取得
- `301`, `302`, `303`, `307`, `308` のリダイレクト追従に対応
- `chunked`, `gzip`, `br` のレスポンスを処理
- `file://` はローカルファイルを読み込み、拡張子から Content-Type を推定
- 画像の取得、デコード、自然サイズ取得、GPU テクスチャ化に対応

### HTML

- 基本的な HTML パース
- `html` / `head` / `body` の補完
- `meta`, `img`, `br`, `link` などの void tag に対応
- `p` の中で block 要素が来た場合の簡易的な閉じ処理
- HTML エンティティのデコード
- `script` / `style` の raw text 扱い
- 属性名とタグ名の小文字化

### CSS

- 最小 UA stylesheet を内蔵
- `<style>` に対応
- `<link rel="stylesheet">` に対応
- `@import` の再帰展開に対応
- CSS コメントのスキップに対応
- `border` shorthand の簡易展開に対応
- セレクタ対応:
  - タグセレクタ
  - `.class`
  - `#id`
  - `tag.class`
  - 子孫セレクタ
  - カンマ区切りセレクタ
- specificity と後勝ち順序を考慮して宣言を適用
- 継承プロパティの一部に対応
- 色指定は `#rgb`, `#rrggbb`, `rgb()`, `rgba()`, 基本的な named color に対応

### レイアウト

- block / inline レイアウト
- inline formatting context の簡易実装
- anonymous block box による inline 要素の流し込み
- ネストした inline 要素の同一行レイアウト
- テキストの空白畳み込みと折り返し
- 長い単語の文字単位折り返し
- `margin`, `padding`, `border-width`, `border-radius`
- `width`, `height`, `margin: auto` の一部
- `position: relative` / `absolute` / `fixed` と `top` / `right` / `bottom` / `left` / `inset`
- `float: left` / `right` と `clear: left` / `right` / `both` の簡易実装
- stacking context ベースの positioned 要素 `z-index` 重ね順制御
- 画像の CSS サイズ、HTML 属性サイズ、自然サイズの反映
- `px`, `%`, `vw`, `vh` の長さ解釈

### 描画/操作

- テキスト描画
- 背景色描画
- border 描画
- border-radius 描画
- リスト bullet 描画
- 画像描画
- 画像読み込み失敗時の `alt` テキスト表示
- リンク下線描画
- 複数 fragment に分かれた同一リンク下線の結合
- リンク hover 時の色変化
- リンク上でのカーソル変更
- クリックによるページ遷移
- マウスホイールによるスクロール
- ウィンドウリサイズ時の再レイアウト

## 技術的な見どころ

- `std::net` と `native-tls` を使い、ブラウザ向けの最小限の HTTP/HTTPS 取得を実装
- HTTP レスポンスの header/body 分離、chunked decode、gzip/br decode を自前で処理
- DOM 正規化により、不完全な HTML でも `html/head/body` を持つ構造へ補完
- CSS の selector matching に specificity を導入
- inherited property を style tree 構築時に子孫へ伝播
- anonymous block box を使って inline 要素の流し込みを実装
- `fontdue` のメトリクスで文字幅を測り、折り返しと描画位置を計算
- `fontdue` でグリフを rasterize し、glyph atlas を使ってテキスト描画
- `wgpu` + WGSL により、矩形、border、画像、文字を個別パイプラインで描画
- 画像は自然サイズ取得、失敗キャッシュ、GPU テクスチャキャッシュを持つ構成

## 使用技術

- Language: Rust (Edition 2024)
- Window/Event: `winit`
- Rendering: `wgpu`, WGSL
- Text rasterization: `fontdue`
- TLS: `native-tls`
- Compression: `flate2`, `brotli`
- Image decoding: `image`

## 現在の制限

学習・実験用の実装のため、以下はまだ未対応です。

- JavaScript 実行
- フォーム送信やアプリケーション的なページ挙動
- アドレスバー、タブ、戻る/進む、履歴などのブラウザ UI
- Cookie、Storage、Cache などの永続化機能
- 実ブラウザ水準の HTML/CSS 互換性
- HTTP/2、HTTP/3、ストリーミング描画
- 文字コード判定や本格的な font fallback
- クロスプラットフォームなフォント読み込み

また、CSS は最小実装のため次のようなものは未対応です。

- 子セレクタ (`>`)
- 隣接兄弟セレクタ (`+`)
- 属性セレクタ (`[attr]`)
- 疑似クラス/疑似要素
- `@media` など多くの at-rule
- flex / grid / table layout
- `position: sticky` のスクロール追従
- 実ブラウザ水準の複雑な float 回り込み
- `opacity` / `transform` などを含む実ブラウザ水準の stacking context 生成条件
- inline 要素の border 描画
- `font-family` に応じた実フォント切り替え

## 実行方法

URL を第 1 引数として渡して起動します。

### リモートページ

```bash
cargo run https://example.com
```

### ローカルファイル

```bash
cargo run file:///D:/path/to/page.html
```

起動すると次のタイトルのネイティブウィンドウが開きます。

```text
Rust Browser (winit0.29 + wgpu0.19)
```

## 動作環境メモ

現状は Windows 前提の実装です。

- `src/main.rs` で `C:\Windows\Fonts\meiryo.ttc` を直接読んでいます
- `src/gpu.rs` では Windows の日本語フォントを複数候補から探します
- `assets/DejaVuSans.ttf` は主にテスト用フォントとして使っています

そのため、非 Windows 環境で動かすにはフォント読み込み部分の修正が必要です。

## ディレクトリ構成

- `src/main.rs`
  - エントリーポイント
  - ページ構築
  - イベントループ
  - hover / click / scroll / resize 処理
- `src/constants.rs`
  - ブラウザ、描画、レイアウト、ネットワーク関連の定数
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
  - リンク下線や画像 fallback の処理
- `src/render.rs`
  - display list を GPU 描画へ渡す薄いレンダリング入口
- `src/gpu.rs`
  - GPU パイプライン
  - glyph atlas
  - 画像テクスチャキャッシュ
  - 描画処理
- `src/image_loader.rs`
  - 画像取得
  - デコード
  - 自然サイズ取得
- `src/utility/`
  - URL 正規化などの補助処理
- `src/*.wgsl`
  - 矩形、文字、画像描画用シェーダ
- `src/html/`
  - ローカル確認用のサンプル HTML と画像
- `assets/`
  - テスト用フォントなどの補助アセット

## 補足

`src/html/` 配下にはローカル確認用のサンプル HTML を含めています。

このリポジトリは完成品のブラウザというより、ブラウザ内部の仕組みを Rust で
段階的に実装し、その理解と実装力を示すためのプロジェクトです。
