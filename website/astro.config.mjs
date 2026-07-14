import { defineConfig } from "astro/config";
import sitemap from "@astrojs/sitemap";

export default defineConfig({
  site: "https://grayslate.app",
  output: "static",
  integrations: [sitemap()],
  build: {
    assets: "assets",
  },
});
