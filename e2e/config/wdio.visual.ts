import { makeConfig } from "./wdio.base.js";

/**
 * Typography, theming, and hover-state measurement.
 *
 * Separate from the functional suite on purpose: these assert pixel values,
 * font weights, and computed colours, so they fail whenever the design changes
 * deliberately, and their hover assertions depend on a real window manager
 * being present. Neither property should be able to block a product change.
 */
export const config: WebdriverIO.Config = makeConfig(["visual"]);
