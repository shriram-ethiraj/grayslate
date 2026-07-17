import tailwindcss from "@tailwindcss/vite";
import { defineConfig } from "vite";
import { sveltekit } from "@sveltejs/kit/vite";
import Icons from "unplugin-icons/vite";

const host = process.env.TAURI_DEV_HOST;

/**
 * Serve `~icons/<collection>/<icon>?raw` modules on the Windows dev server.
 *
 * Icons imported with `?raw` (see `markdownAutocomplete.ts`) resolve to the
 * virtual id `~icons/lucide/foo?raw`. Vite applies an fs allow-list check to
 * every `?raw` URL *before* the plugin pipeline runs, and that check hard-denies
 * any path containing `~` on Windows, because `~` is 8.3 short-name syntax
 * (`PROGRA~1`) and a path-traversal vector:
 *
 *     if (isWindows && filePath.includes("~")) return false;
 *
 * The virtual module is not a readable file either, so the check returns
 * "fallback", Vite calls `next()`, and SvelteKit answers the module request with
 * its HTML 404 page — the import then fails and the app dies with a 500. On
 * Linux the tilde branch is skipped and the id is allowed via `safeModulePaths`,
 * which is why this only ever reproduced on Windows.
 *
 * Registering the middleware from `configureServer` puts it ahead of Vite's
 * internal transform middleware, so these ids are transformed normally while the
 * fs allow-list stays enforced (`server.fs.strict` untouched) for every other
 * request. Production builds never hit this path: Rollup calls `load()` directly
 * and inlines the SVG, so this is dev-only.
 *
 * @returns {import("vite").Plugin}
 */
function serveRawIconsOnWindows() {
  const RAW_ICON_URL_RE = /^\/@id\/~icons\/[^/]+\/[^/?]+\?raw\b/;
  /** @type {import("vite").TransformOptions & { skipFsCheck: boolean }} */
  const rawIconTransformOptions = { skipFsCheck: true };

  return {
    name: "grayslate:serve-raw-icons",
    apply: "serve",
    configureServer(server) {
      server.middlewares.use((req, res, next) => {
        const url = req.url;
        if (!url || !RAW_ICON_URL_RE.test(url)) {
          next();
          return;
        }

        // `/@id/` is Vite's wrapper for non-file ids; strip it to get the id
        // the plugin container resolves.
        server
          .transformRequest(url.slice("/@id/".length), rawIconTransformOptions)
          .then((result) => {
            if (!result) {
              next();
              return;
            }
            res.setHeader("Content-Type", "application/javascript");
            res.end(result.code);
          })
          .catch(next);
      });
    },
  };
}

// https://vite.dev/config/
export default defineConfig(async () => ({
  plugins: [
    tailwindcss(),
    sveltekit(),
    Icons({
      compiler: "svelte",
      autoInstall: false,
    }),
    serveRawIconsOnWindows(),
  ],
  // Vite options tailored for Tauri development and only applied in `tauri dev` or `tauri build`
  //
  // 1. prevent Vite from obscuring rust errors
  clearScreen: false,

  // 2. tauri expects a fixed port, fail if that port is not available
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host ? { protocol: "ws", host, port: 1421 } : undefined,
    watch: {
      // 3. tell Vite to ignore watching `src-tauri`
      ignored: ["**/src-tauri/**"]
    }
  }
}));
