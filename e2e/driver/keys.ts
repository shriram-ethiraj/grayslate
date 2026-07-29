import { browser } from "@wdio/globals";

/**
 * WebDriver key codes live in the U+E0xx private-use block. Spelling them as
 * code points keeps the source readable and greppable — the raw characters are
 * invisible in an editor and easy to corrupt in a copy/paste.
 */
const key = (codePoint: number): string => String.fromCharCode(codePoint);

const CONTROL = key(0xe009);
const META = key(0xe03d);

export const NULL = key(0xe000);
export const TAB = key(0xe004);
export const ENTER = key(0xe007);
export const SHIFT = key(0xe008);
export const ALT = key(0xe00a);
export const ESCAPE = key(0xe00c);
export const END = key(0xe010);
export const HOME = key(0xe011);
export const ARROW_LEFT = key(0xe012);
export const ARROW_UP = key(0xe013);
export const ARROW_RIGHT = key(0xe014);
export const ARROW_DOWN = key(0xe015);
export const DELETE = key(0xe017);

/** The platform modifier the app binds "Mod+" shortcuts to. */
export const MOD = process.platform === "darwin" ? META : CONTROL;

/**
 * Type text one character at a time with a full down/up per key.
 *
 * WebKit treats adjacent identical key-downs as auto-repeat, so
 * `browser.keys(string)` silently drops repeated characters. The per-character
 * path preserves the exact bytes, and the 24-character batching keeps each
 * action payload small — WebKitWebDriver can reorder events inside a very long
 * single sequence under load while still reporting success.
 */
export async function typeText(text: string): Promise<void> {
  const batchSize = 24;
  for (let start = 0; start < text.length; start += batchSize) {
    const action = browser.action("key");
    for (const character of text.slice(start, start + batchSize)) {
      const stroke = character === "\n" ? ENTER : character;

      // Key actions send a raw code point: unlike `sendKeys`, they do not
      // synthesize the Shift needed to produce a capital, so `down("S")`
      // arrives as a lowercase "s". Holding Shift explicitly is what makes the
      // typed text match what the caller asked for — silently lowercasing it
      // turns an exact-text assertion into a confusing near-miss.
      const needsShift = character !== character.toLowerCase();
      if (needsShift) action.down(SHIFT);
      action.down(stroke).pause(25).up(stroke).pause(25);
      if (needsShift) action.up(SHIFT);
    }
    await action.perform();
  }
}

/** Press the platform modifier plus a key, e.g. `pressMod("s")` for Save. */
export async function pressMod(stroke: string): Promise<void> {
  await browser.keys([MOD, stroke]);
}

/** Press the platform modifier plus Shift plus a key. */
/**
 * Force every modifier back up.
 *
 * `browser.keys([MOD, "v"])` is supposed to release what it pressed, but the
 * keyup is delivered to whatever holds focus at that moment. If focus moves or
 * the focused node is detached in between, the webview never observes the
 * release and behaves as though the modifier is still held. An explicit keyUp
 * for each modifier is unconditional, and harmless when nothing is held.
 */
export async function releaseModifiers(): Promise<void> {
  // NULL (U+E000) is defined by WebDriver as "release all currently pressed
  // modifier keys". `releaseActions()` alone was not enough here: it only
  // unwinds state the driver believes it holds, and the stuck modifier came
  // from a keyup delivered to a node that had already been detached.
  await browser.keys(NULL).catch(() => {});
  await browser.releaseActions().catch(() => {});
}

export async function pressModShift(stroke: string): Promise<void> {
  await browser.keys([MOD, SHIFT, stroke]);
}
