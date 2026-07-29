import { makeConfig } from "./wdio.base.js";

/**
 * Packaged trust-boundary checks.
 *
 * Security has its own blocking CI job so a functional failure cannot prevent
 * capability, CSP, document-authorization, and hostile-content probes from
 * running and reporting their independent result.
 */
export const config: WebdriverIO.Config = makeConfig(["security"]);
