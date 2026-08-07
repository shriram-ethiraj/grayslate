import { makeConfig } from "./config/wdio.base.js";

/**
 * The default entry point: every suite, in tier order (functional, visual,
 * security).
 *
 * Specs are discovered from `e2e/specs/` rather than hand-listed, so a new spec
 * file runs as soon as it exists. Run one tier with
 * `wdio run e2e/wdio.conf.ts --suite functional`, or use the dedicated configs
 * in `e2e/config/` when a CI job should own a single tier.
 */
export const config: WebdriverIO.Config = makeConfig();
