# cargo-heu

ヒューリスティックコンテスト向けのビルド・実行・ビジュアライザ評価ハーネス。

`cargo-heu` は、ローカルでの大量ケース実行とスコア確認を効率化するための CLI ツールです。  
AtCoder Heuristic Contest などで、ビルドから評価までを一括で回す用途を想定しています。

## 主な機能

- `[build]` 設定に基づくビルド実行（`enable` / `command`）
- 複数ケース実行（例: `0-9`, `0 1 3-5`）
- 並列実行（`threads`）
- ビジュアライザ出力から `Score = <num>` を抽出
- `stderr` の `# ` プレフィックス行をコメントとして抽出表示
- 最後に処理したケースの出力をクリップボードへコピー
- `--no-evaluate`（評価スキップ）をサポート
- `test.use_tester=true` による tester 経由実行をサポート

## 前提環境

- Rust / Cargo が使えること
- 入力ファイルが `./tools/in/0000.txt` 形式で配置されていること
- 出力先ディレクトリ（既定: `./tools/out/`）を利用できること
- ビジュアライザ実行コマンドが参照する `tools/Cargo.toml` などが必要に応じて用意されていること

## インストール

このリポジトリ直下で:

```bash
cargo install --path .
```

インストール後は次の形式で実行できます。

- `cargo-heu`（直接実行）
- `cargo heu`（cargo サブコマンドとして実行）

## クイックスタート

初回実行時に `heu.toml` が存在しない場合、デフォルト設定ファイルが自動生成されます。

```bash
# デフォルト設定で実行
cargo heu

# ケース 0-9 を 8 スレッドで実行
cargo heu 0-9 -j 8

# ケース指定 + 評価なし実行
cargo heu 0 1 3-5 --no-evaluate
```

出力イメージ:

```text
0000 SCORE[     12,345] ELAPSED[0.12s] CMTS[init/ok]
0001 SCORE[     23,456] ELAPSED[0.10s] CMTS[]
...
TOTAL=123,456
```

## 設定ファイル（`heu.toml`）

### 主なキー

| Key | 説明 |
|---|---|
| `build.enable` | `true` なら実行前にビルドを行う |
| `build.command` | 実行するビルドコマンド |
| `test.bin` | 実行対象バイナリ（またはコマンド） |
| `test.cases` | ケース指定文字列（例: `"0-9"`） |
| `test.threads` | 並列実行スレッド数 |
| `test.no_evaluate` | `true` ならビジュアライザ評価をスキップ |
| `test.use_tester` | `true` なら tester コマンド経由で実行 |
| `test.in_dir` | 入力ファイルディレクトリ |
| `test.out_dir` | 出力ファイルディレクトリ |
| `test.vis` | ビジュアライザ実行コマンド |
| `test.tester` | tester 実行コマンド |
| `test.score_regex` | ビジュアライザ出力からスコアを抽出する正規表現（第1キャプチャ） |
| `test.comment_regex` | `stderr` の各行からコメントを抽出する正規表現（第1キャプチャ） |

### サンプル

```toml
[build]
enable = true
command = "cargo build --release --bin a --target-dir target -q"

[test]
bin = "./target/release/a"
cases = "0-9"
threads = 8
no_evaluate = false
use_tester = false
in_dir = "./tools/in"
out_dir = "./tools/out"
vis = "cargo run --manifest-path tools/Cargo.toml --bin vis --target-dir=tools/target -r"
tester = "cargo run --manifest-path tools/Cargo.toml --bin tester --target-dir=tools/target -r"
score_regex = "Score = (\\d+)"
comment_regex = "^# (.*)$"
```

既定値では、スコアは `Score = <num>` 形式、コメントは `# ` で始まる行を抽出します。

## CLI オプション

- `-f, --config <PATH>`: 設定ファイルパス（デフォルト `./heu.toml`）
- `-j, --threads <N>`: 並列スレッド数
- `-n, --no-evaluate`: 評価なしで実行
- `cases...`: ケース指定（例: `0`, `3-5`, `0 1 3-5`）

## Troubleshooting

- `heu.toml` が見つからない:
  - `cargo heu` を実行して自動生成する
  - `-f` 指定時はパス誤りがないか確認する
- `heu.toml` のパースに失敗する:
  - TOML 構文ミス（クオート、真偽値、セクション名）を確認する
- `tools/in` に入力がない:
  - `test.in_dir` と実ファイル配置が一致しているか確認する
- 出力ファイルが作られない:
  - `test.out_dir` のパスが正しいか、作成権限があるか確認する
- `build.command` や `test.bin` が失敗する:
  - 実行パス、ビルドターゲット、実行権限を確認する
- `vis` / `tester` が失敗する:
  - コマンド文字列、`tools/Cargo.toml` の存在、依存バイナリのビルド状態を確認する

## 開発向けメモ

ローカル確認:

```bash
cargo test
cargo check
```

主要ファイル:

- `src/main.rs`: CLI 引数処理と設定読み込み
- `src/lib.rs`: ケース実行、並列処理、スコア抽出ロジック

## 補足

- Public API / CLI / 設定フォーマットの変更はありません（ドキュメント追加のみ）。
- ライセンスおよびコントリビューション方針は、必要に応じて別途追記してください。
