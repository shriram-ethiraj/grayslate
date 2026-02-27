# Grayslate

**Grayslate** is a simple yet powerful code and data editor built using **Tauri**, **SvelteKit**, and **Rust**. The main goal of this project is to provide a fast and clean tool for editing and viewing data.

## ✨ Key Features

-   **🚀 Fast Data Viewing**: High-performance CSV table view that can open large files without much trouble.
-   **🔍 Automatic Language Detection**: It can detect around 15+ languages like JSON, Python, and CSV automatically using simple heuristics and ML.
-   **🎨 Clean UI**: A simple and tidy interface built with **Tailwind CSS v4** and **Shadcn**. It is easy on the eyes and includes a dark mode as well.
-   **🛠️ Robust Editor**: Uses **CodeMirror 6** to provide a solid editing experience for everyday tasks.
-   **📦 Native App**: Works well on macOS, Windows, and Linux as a proper desktop application using **Tauri 2.0**.

## 🛠️ Tech Stack

-   **Frontend**: [SvelteKit](https://kit.svelte.dev/) + [Svelte 5](https://svelte.dev/)
-   **Styling**: [Tailwind CSS v4](https://tailwindcss.com/)
-   **Editor**: [CodeMirror 6](https://codemirror.net/)
-   **Backend**: [Tauri 2.0](https://tauri.app/) (Rust)

## 🚀 Getting Started

Kindly follow these steps to set up the project on your local machine.

### Prerequisites

Please ensure you have these installed:
-   [Node.js](https://nodejs.org/) (v18 or above)
-   [Rust](https://www.rust-lang.org/) (latest version)
-   [pnpm](https://pnpm.io/)

### Development

1.  **Clone the repository**:
    ```bash
    git clone https://github.com/shriramethiraj/grayslate.git
    cd grayslate
    ```

2.  **Install dependencies**:
    ```bash
    pnpm install
    ```

3.  **Run the application**:
    ```bash
    pnpm tauri dev
    ```

### Building the Project

To create a final build for your system, you can run:
```bash
pnpm tauri build
```

## 🤝 Contributing

We welcome any kind of help! If you find any issues or want to suggest updates, please feel free to open an issue or a pull request.

## 📄 License

This project is available under the **MIT License**.

---

*Made with care by [Shriram Ethiraj](https://github.com/shriramethiraj)*
