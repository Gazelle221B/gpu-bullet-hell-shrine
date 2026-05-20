# 霊符演算録 〜 GPU Bullet Hell Shrine - 開発ガイド・エージェント指示書

本ファイルは、プロジェクトのビルド、実行、および AI エージェントに対する開発ルールを一元管理するファイルです。

---

## ⚠️ AI エージェントへの重要指示 (Critical Instructions)

1. **ビルド制限（サンドボックス環境の制約）**
   - 本開発環境は macOS のサンドボックス環境で動作しており、標準の `cargo` や `rustc` を直接実行すると動的ライブラリロードでエラーが発生する可能性があります。
   - Rust から WebAssembly へのコンパイルには、**必ずプロジェクトルートにある `./build.sh` を使用してください**。
   - `./build.sh` は内部で `.bin` にあるカスタムラッパーを `PATH` に追加し、オフラインモードで `wasm-pack` を呼び出します。直接 `wasm-pack` を実行しないでください。

2. **コマンドの実行方法**
   - フロントエンド依存関係のインストール: `npm install`
   - Wasm ビルド: `./build.sh`
   - ローカル開発サーバー起動: `npm run dev`
   - フロントエンドビルド: `npm run build`

---

## 🛠️ プロジェクト構成 (Architecture)

プロジェクトは Rust (Wasm) + WebGPU (WGSL) + Vite/TypeScript のマルチクレイツ構成です。

- **`crates/app`** (Wasm Bridge)
  - Rust から JavaScript へゲーム状態、制御インターフェース、パフォーマンスメトリクスを露出するブリッジ。
- **`crates/compute`** (WebGPU Compute Shaders)
  - WGSL を使用した弾道計算、スペルカードロジック、衝突判定用の空間ハッシュ（Spatial Hashing）の処理。
- **`crates/render`** (WebGPU Graphics Pipeline)
  - インスタンス描画および加算合成を用いたパーティクル・弾幕描画パイプライン。
- **`crates/game`** (CPU-side State & Logic)
  - CPU 側での境界判定、時間管理、ライフやボムなどのプレイヤー状態管理。
- **`crates/shared`** (Common Memory Models)
  - Rust と WGSL コンピュートシェーダー間で共有される構造体。`std140` や `std430` のレイアウトアライメント制約に準拠するデータモデル。
- **`web`** (Frontend UI/UX)
  - TypeScript、HTML、および CSS によるフロントエンド。WebAssembly モジュールをロードし、ガラスモーフィズム（Glassmorphism）スタイルの HUD を描画。

---

## 📝 コーディング規約・開発方針 (Coding Guidelines)

### 1. WebGPU / WGSL & メモリレイアウト
- Rust と WGSL の間で共有する構造体は、アライメントルール（`std140` / `std430`）を厳格に順拠してください。
  - `vec3` を使用する際は、4ワード境界（16バイト）アライメントに注意し、アライメントパディングを含めるか、代わりに `[f32; 4]` や `vec4` を使用することを推奨します。
  - `crates/shared` でアライメントを制御した定義を行います。

### 2. Rust (WebAssembly)
- パフォーマンス向上のため、メインのアップデートループ内での動的メモリ確保（`Vec` のリアロケーションや `String` 生成など）を最小限に抑えてください。
- JavaScript との境界でのデータのやり取りは、可能な限りポインタや共有メモリバッファ（Float32Array など）を通じてゼロコピーで行います。

### 3. フロントエンド (CSS & UI)
- CSS は Vanilla CSS を使用し、非常に美しく洗練されたデザイン（ガラスモーフィズム、ネオン調のグラデーション、滑らかなマイクロアニメーション）を維持してください。
- パフォーマンス表示（FPS、GPU負荷、弾数、メモリ）を表示する HUD は、ハードウェアのリアルタイム情報を反映するようにします。

### 4. Git コミット
- コミット前に必ず `./build.sh` が通り、`npm run build` がエラーなく完了することを確認してください。
- `GDD.md` はゲームデザインドキュメントですが、Git の管理対象から意図的に外されているため、コミットに含めないでください。
