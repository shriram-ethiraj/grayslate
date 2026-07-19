import { defineConfig } from "astro/config";
import sitemap from "@astrojs/sitemap";

export default defineConfig({
  site: "https://grayslate.app",
  output: "static",
  integrations: [sitemap()],
  build: {
    assets: "assets",
  },
  vite: {
    build: {
      // Keep scripts external so the strict CSP can authorize them through `script-src 'self'`.
      // They are content-hashed and receive the immutable /assets/* cache policy.
      assetsInlineLimit: 0,
    },
  },
});
