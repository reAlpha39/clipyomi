# ClipYomi

**ClipYomi** is a fast, lightweight Japanese reading companion and clipboard parsing HUD for visual novels, games, and immersion reading.

It is a cross-platform desktop app (Rust + Tauri v2 + TypeScript) built around its own Japanese parsing engine. Unlike full translation suites, **ClipYomi contains zero machine translation**—it focuses exclusively on morphological text segmentation, conjugation deinflection, furigana readings, and instant dictionary lookups.

---

## Architecture

The workspace is organized into modular Rust crates and a modern web frontend:

```text
├── crates/
│   ├── jparser/         # Pure, reusable Japanese segmentation & conjugation parser
│   ├── jmdict-source/   # JMDict download, staging, and indexing pipeline
│   └── vibrato-source/  # Pinned IPADIC download for the optional boundary hints
├── src-tauri/           # Tauri v2 desktop shell, clipboard monitoring, & window management
└── src/                 # TypeScript + Vite UI (popover positioning, DOM rendering, styling)
```

---

## Getting Started

### Prerequisites

1. **Rust:** Latest stable Rust. The workspace floor is **1.89** — that is `vibrato-source`'s `rust-version`, and `cargo test --workspace` below is refused by cargo under it. Individually: `jparser` and `jmdict-source` 1.85, `src-tauri` 1.88, `vibrato-source` 1.89.
   ```bash
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   ```
2. **Node.js:** v18+ and npm.

---

### Installation & Development

1. **Clone the repository:**

   ```bash
   git clone https://github.com/reAlpha39/clipyomi.git
   cd clipyomi
   ```
2. **Install frontend dependencies:**

   ```bash
   npm install
   ```
3. **Run in development mode (Tauri + Vite hot-reload):**

   ```bash
   npm run tauri dev
   ```
4. **First Run Setup:**
   On first launch, click **Download Dictionary** in the app to fetch and build the local JMDict search index. Once built, parsing is instantaneous and fully offline.

---

## Testing

Run frontend unit and E2E tests:

```bash
npm test          # Run Vitest unit tests
npm run test:e2e  # Run Playwright E2E tests
```

Run Rust crate tests:

```bash
cargo test --workspace
```

---

## Tech Stack

- **Desktop Framework:** [Tauri v2](https://v2.tauri.app/)
- **Backend Language:** [Rust](https://www.rust-lang.org/)
- **Frontend:** [TypeScript](https://www.typescriptlang.org/), [Vite](https://vitejs.dev/)
- **Morphological Tokenizer:** [Vibrato](https://github.com/daac-tools/vibrato) (optional MeCab backend)
- **Dictionary:** [JMdict](https://www.edrdg.org/jmdict/j_jmdict.html) and [KANJIDIC2](https://www.edrdg.org/wiki/index.php/KANJIDIC_Project) (EDRDG)

---

## License & Acknowledgments

- **ClipYomi** is dual-licensed under **MIT OR Apache-2.0** (`LICENSE-MIT`, `LICENSE-APACHE`).
- Dictionary data from the **EDRDG**'s JMdict and KANJIDIC2 is CC BY-SA 4.0 under its own notice (`crates/jparser/tests/fixtures/README.md`); the **IPADIC** conjugation data carries its notice in `crates/jparser/assets/IPADIC_NOTICE`.
