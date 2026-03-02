---
name: Svelte Frontend Guidelines
description: Rules for writing Svelte 5, strictly typed TypeScript, and Vite configuration in Grayslate.
---

# Svelte Frontend Guidelines

## Core Principles

- **Embrace Svelte 5 Runes:** Exclusively use modern Svelte 5 signals (Runes).
  - Use `$state()` for reactive state.
  - Use `$derived()` for computed values.
  - Use `$effect()` for side effects.
  - Use `$props()` instead of `export let` for component inputs.
  - **Avoid legacy Svelte 4 features** (`$:`, legacy slot architecture). Opt for Svelte 5 `{#snippet}` when handling template injection.
- **Strong Typing:** Do not use `any`. Define strictly typed interfaces and types for all component props, Tauri IPC payloads, and CodeMirror extensions.
- **Vite Native:** Keep assets optimized. Import static assets cleanly and let Vite handle caching and bundling.
