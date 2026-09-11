# ClipYomi

**ClipYomi** is a fast, lightweight Japanese reading companion and clipboard parsing HUD for visual novels, games, and immersion reading.

It is a modern cross-platform (Rust + Tauri v2 + TypeScript) rewrite of the classic **JParser** engine from [Translation Aggregator](https://github.com/Translation-Aggregator/Translation-Aggregator). Unlike full translation suites, **ClipYomi contains zero machine translation**—it focuses exclusively on morphological text segmentation, conjugation deinflection, furigana readings, and instant dictionary lookups.

---

## Features

- **Seamless Clipboard Monitoring:** Automatically captures Japanese text from text hookers (Textractor, Agent, ITHVNR), optical character recognition (OCR), or standard clipboard copying.
- **Built-in JMDict Integration:** Automatically downloads and compiles an optimized, memory-mapped local index of the JMDict multilingual Japanese dictionary.
- **Interactive Floating Popovers:**
  - Full grammatical breakdown with conjugated forms (e.g. *Past*, *Causative-Passive*, *Polite Negative*).
  - Furigana readings, pitch/romaji support, and part-of-speech tags.
  - Rich definitions, glosses, and cross-references.
- **Immersion HUD & Desktop Overlay:**
  - **Always on Top** pinning for overlaying games or readers.
  - **Frameless / Minimalist** title bar toggling.
  - Responsive, keyboard-accessible, and dark-theme friendly UI.
- **Cross-Platform:** Native support for macOS and Windows.

---

## Architecture

The workspace is organized into modular Rust crates and a modern web frontend:

```text
├── crates/
│   ├── jparser/         # Pure, reusable Japanese segmentation & conjugation parser
│   └── jmdict-source/   # JMDict download, staging, and indexing pipeline
├── src-tauri/           # Tauri v2 desktop shell, clipboard monitoring, & window management
└── src/                 # TypeScript + Vite UI (popover positioning, DOM rendering, styling)
```

---

## Getting Started

### Prerequisites

1. **Rust:** Latest stable Rust (1.88+ recommended, 1.85+ for `jparser` crate).
   ```bash
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   ```
2. **Node.js:** v18+ and npm.

---

### Installation & Development

1. **Clone the repository:**
   ```bash
   git clone https://github.com/your-username/clipyomi.git
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

- **ClipYomi** is dual-licensed under **MIT OR Apache-2.0** (`LICENSE-MIT`, `LICENSE-APACHE`). The parsing engine is a clean-room implementation; see `docs/superpowers/specs/2026-08-27-engine-v2-master-plan.md` §3.5.
- Dictionary data from the **EDRDG**'s JMdict and KANJIDIC2 is CC BY-SA 4.0 under its own notice (`crates/jparser/tests/fixtures/README.md`); the **IPADIC** conjugation data carries its notice in `crates/jparser/assets/IPADIC_NOTICE`.
- Dedicated to the authors and contributors of **Translation Aggregator** (Hongfire / Sinflower / TA contributors) for the original JParser design.
