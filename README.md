<div align="center">
  <img src="app-icon.png" alt="Grayslate Logo" width="128" />
  <h1>Grayslate</h1>
  <p><strongA fast scratchpad for code, data, and quick thinking.</strong></p>
  
  <p>
    <a href="https://github.com/shriram-ethiraj/grayslate/blob/main/LICENSE">
      <img src="https://img.shields.io/github/license/shriram-ethiraj/grayslate?style=flat-square" alt="License" />
    </a>
    <a href="https://github.com/shriram-ethiraj/grayslate/issues">
      <img src="https://img.shields.io/github/issues/shriram-ethiraj/grayslate?style=flat-square" alt="Issues" />
    </a>
    <a href="https://github.com/shriram-ethiraj/grayslate/stargazers">
      <img src="https://img.shields.io/github/stars/shriram-ethiraj/grayslate?style=flat-square" alt="Stars" />
    </a>
  </p>
</div>

**Grayslate** is a simple yet powerful cross-platform code and data editor. Designed as a lightweight scratchpad and data viewer, it provides a fast and clean environment for everyday text manipulation, data viewing, and code editing without the bloat of a full IDE.

---

## ✨ Key Features

- **🚀 Fast Data Viewing**: High-performance CSV table view that can open large files without breaking a sweat.
- **🔍 Automatic Language Detection**: Automatically detects 15+ languages (like JSON, Python, and CSV) using smart heuristics.
- **🎨 Clean UI**: A tidy, distraction-free interface built with **Tailwind CSS v4** and **Shadcn**. Includes a beautiful dark mode.
- **🛠️ Robust Editor**: Powered by **CodeMirror 6** for a solid, modern editing experience.
- **📦 Cross-Platform**: Runs on macOS, Windows, and Linux as a dedicated desktop application using **Tauri 2.0**.
- **🔒 Privacy First**: Everything runs locally on your machine. No cloud syncing, no data collection.

## 🛠️ Tech Stack

Grayslate is built using modern, high-performance tools to remain fast, small, and reliable:

- **Frontend**: [SvelteKit](https://kit.svelte.dev/) + [Svelte 5](https://svelte.dev/)
- **Styling**: [Tailwind CSS v4](https://tailwindcss.com/) + Shadcn
- **Editor Core**: [CodeMirror 6](https://codemirror.net/)
- **Backend**: [Tauri 2.0](https://tauri.app/) (Rust)

## 🚀 Getting Started

Want to build Grayslate from source? Follow these steps to set up the project on your local machine.

### Prerequisites

Please ensure you have these installed:

- [Node.js](https://nodejs.org/) (v18 or above)
- [Rust](https://www.rust-lang.org/) (latest version)
- [pnpm](https://pnpm.io/)

### Local Development

1. **Clone the repository**:

   ```bash
   git clone https://github.com/shriram-ethiraj/grayslate.git
   cd grayslate
   ```

2. **Install dependencies**:

   ```bash
   pnpm install
   ```

3. **Run the application**:
   ```bash
   pnpm tauri dev
   ```

### Building the Project

To create a final optimized build for your operating system:

```bash
pnpm tauri build
```

## ❓ Frequently Asked Questions (FAQ)

### Is this similar to Boop or Notepad++?

Yes and no. Like **Boop**, Grayslate serves as an excellent developer scratchpad for quick manipulations, but goes further by offering robust file editing and high-performance data viewing (like large CSVs). Unlike **Notepad++**, which is Windows-only, Grayslate is truly cross-platform out of the box with a modern, clean UI, taking the best ideas from both worlds and bringing them to macOS, Windows, and Linux.

### Why use Tauri instead of Electron?

**Performance and bundle size.** Electron bundles a whole Chromium browser and Node.js runtime, making apps huge and memory-hungry. By using Tauri, Grayslate relies on the OS's system webview and a lightweight Rust backend. This results in significantly lower RAM usage, vastly smaller app bundles, and excellent performance.

### Why not just use online text/data converters?

**Privacy.** Online formatters and converters require you to send your data over the internet to a third-party server. Whether it's proprietary code, secret API keys, or sensitive CSV data, you shouldn't have to risk exposing it just to format JSON. Grayslate processes everything 100% locally on your machine.

### Why not use VS Code, IntelliJ, or another heavyweight IDE?

IDEs are fantastic for managing large projects, but they can be slow to launch and overly complex when all you want to do is paste an API payload, quickly format it, or view a CSV file. Grayslate is designed to open instantly, provide the core tools you need without the bloat, and get out of your way.

### Can Grayslate handle extremely large files?

Yes. Grayslate is specifically optimized for viewing large datasets. The CSV viewer uses a high-performance table implementation that can handle hundreds of thousands of rows without slowing down your system. For text editing, it uses **CodeMirror 6**, which is designed for fast, efficient manipulation of large text blocks.

### Is Grayslate free to use?

Yes! Grayslate is completely free and open-source.

## 🤝 Contributing

Contributions are welcome! If you find any issues, want to request features, or suggest updates, please feel free to open an issue or submit a pull request. Make sure to read the contributing guidelines before jumping in.

## 📄 License

This project is available under the **MIT License**.
