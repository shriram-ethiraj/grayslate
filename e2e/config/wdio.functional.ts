import { makeConfig } from "./wdio.base.js";

/**
 * The blocking CI job: functional and lifecycle behavior only.
 *
 * Visual/typography measurement lives in `wdio.visual.ts` because it fails for
 * a different reason (design churn, hover rendering under a headless WM) and
 * should not gate a product change.
 */
export const config: WebdriverIO.Config = makeConfig(["functional"]);
