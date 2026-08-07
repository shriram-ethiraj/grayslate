import { TIMEOUTS, INTERVALS } from "../config/timeouts.js";
import { pressMod } from "../driver/keys.js";
import { waitFor } from "../driver/wait.js";
import {
  attributeOf,
  byTestId,
  clickTestId,
  isDisplayedTestId,
  textOf,
  waitForTestId,
} from "./common.js";

/**
 * The find/replace panel.
 *
 * Match counting is Rust-backed (`editor_find_scan`), so the count lands
 * asynchronously after the query settles — always wait for it rather than
 * reading it straight after typing.
 */

export async function open(): Promise<void> {
  await pressMod("f");
  await waitForTestId("find-replace-panel");
}

export async function openReplace(): Promise<void> {
  await pressMod("h");
  await waitForTestId("find-replace-panel");
}

export async function close(): Promise<void> {
  await waitForTestId("find-replace-panel", { reverse: true });
}

export async function isOpen(): Promise<boolean> {
  return isDisplayedTestId("find-replace-panel");
}

export async function setQuery(query: string): Promise<void> {
  const input = await byTestId("find-input");
  await input.waitForDisplayed({
    timeout: TIMEOUTS.ui,
    timeoutMsg: "The find input never appeared.",
  });
  await input.setValue(query);
}

export async function setReplacement(value: string): Promise<void> {
  const input = await byTestId("replace-input");
  await input.waitForDisplayed({
    timeout: TIMEOUTS.ui,
    timeoutMsg: "The replace input never appeared.",
  });
  await input.setValue(value);
}

export async function toggleOption(option: "case" | "word" | "regex"): Promise<void> {
  await clickTestId(`find-opt-${option}`);
}

export async function optionPressed(option: "case" | "word" | "regex"): Promise<boolean> {
  return (await attributeOf(`find-opt-${option}`, "aria-pressed")) === "true";
}

export async function matchCountText(): Promise<string> {
  return textOf("find-match-count");
}

/**
 * Wait for the reported match count.
 *
 * The panel renders either `n/total` or a `total+` form for very large
 * documents, so match on the total rather than the whole label.
 */
export async function waitForMatchCount(expected: number): Promise<void> {
  let observed = "";
  await waitFor(
    async () => {
      observed = (await matchCountText()).trim();
      return (
        observed === `${expected}+` ||
        observed.endsWith(`/${expected}`) ||
        observed === String(expected)
      );
    },
    {
      message: () => `Match count never became ${expected}. Last observed: ${JSON.stringify(observed)}`,
      timeoutMs: TIMEOUTS.ui,
      intervalMs: INTERVALS.fast,
    },
  );
}

export async function next(): Promise<void> {
  await clickTestId("find-next");
}

export async function previous(): Promise<void> {
  await clickTestId("find-prev");
}

export async function replaceOne(): Promise<void> {
  await clickTestId("find-replace-one");
}

export async function replaceAll(): Promise<void> {
  await clickTestId("find-replace-all");
}

/**
 * Whether the panel is reporting an invalid regular expression.
 *
 * The panel says so in the match-count slot, not on the input: there is no
 * `aria-invalid` anywhere in `FindReplace.svelte`. Reading an attribute the
 * component never sets meant this returned `false` for every input, valid or
 * not, so it could only ever have confirmed the absence of an error.
 */
export async function hasRegexError(): Promise<boolean> {
  return (await matchCountText()).trim() === "Regex error";
}

/**
 * The current match position, or `null` when no match is active.
 *
 * The label is `current/total` only once a match is selected; before that it
 * renders `total+` (`currentMatch === 0`). That distinction matters because
 * Replace acts on the *current* match, so with no active match it is a no-op.
 */
export async function currentMatchIndex(): Promise<number | null> {
  const [current, total] = (await matchCountText()).trim().split("/");
  if (total === undefined) return null;
  const parsed = Number.parseInt(current ?? "", 10);
  return Number.isFinite(parsed) ? parsed : null;
}

/** Wait until a match is selected, so Replace and navigation act on something. */
export async function waitForCurrentMatch(expected?: number): Promise<void> {
  let observed: string = "";
  await waitFor(
    async () => {
      observed = (await matchCountText()).trim();
      const index = await currentMatchIndex();
      return index !== null && (expected === undefined || index === expected);
    },
    {
      message: () =>
        `The find panel never selected ${expected === undefined ? "a match" : `match ${expected}`}. ` +
        `Last observed label: ${JSON.stringify(observed)}`,
      timeoutMs: TIMEOUTS.ui,
      intervalMs: INTERVALS.fast,
    },
  );
}

/**
 * Whether a panel control is actually actionable.
 *
 * `TooltipButton` marks itself with `aria-disabled` and returns early from its
 * own click handler rather than setting the native `disabled` property. A
 * WebDriver click therefore *succeeds* and does nothing at all, so a test that
 * clicks a disabled control fails later with a confusing symptom instead of at
 * the click. Assert this first.
 */
export async function isActionable(
  control: "find-next" | "find-prev" | "find-replace-one" | "find-replace-all",
): Promise<boolean> {
  return (await attributeOf(control, "aria-disabled")) !== "true";
}
