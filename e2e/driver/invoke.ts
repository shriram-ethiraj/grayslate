import { browser } from "@wdio/globals";

/**
 * The single Tauri IPC bridge for the whole suite.
 *
 * WebDriver evaluates scripts in the webview context, where
 * `__TAURI_INTERNALS__` is reachable — the same access path a hostile page
 * would have, which is why the security specs use it too. Before this module
 * the wrapper existed three times (`helpers/app.ts`, `security/ipc-capabilities`,
 * `security/document-authorization`) with three different error shapes.
 *
 * Direct use is restricted by `e2e/scripts/lint-conventions.mjs`: UI flows must
 * be driven through the UI. IPC is permitted only for the `e2e_*` shims, the
 * security specs, and read-only oracles annotated with `// ipc-oracle:`.
 */
interface TauriInternals {
  invoke<T>(command: string, args?: Record<string, unknown>): Promise<T>;
}

/**
 * How an invoke settled.
 *
 * `rejected` and `transport` must stay distinct. Collapsing them — as the three
 * previous copies of this wrapper did — means a WebDriver failure is
 * indistinguishable from the backend refusing a forged grant, so a security
 * assertion can pass because the driver broke rather than because the
 * capability system worked.
 */
export type InvokeOutcome<T> =
  | { status: "ok"; value: T }
  | { status: "rejected"; message: string }
  | { status: "transport"; message: string };

/**
 * The shape the injected script hands back.
 *
 * The failure field is deliberately NOT called `error`: `@wdio/tauri-service`
 * patches `execute` and re-throws any result carrying an `error` key, which
 * turns an ordinary backend rejection into a driver-level exception and
 * destroys structured error payloads along the way (`{kind, message}` arrives
 * as the string "[object Object]"). Naming it `failure` keeps the result a
 * result.
 */
export interface InvokeResult<T> {
  value?: T;
  failure?: string;
}

/** The camelCase descriptor returned by the open/save grant commands. */
export interface DocumentDescriptor {
  documentId: string;
  generation: number;
  displayPath: string;
  fileName: string;
  source: string;
  writable: boolean;
}

/**
 * Failures that mean the driver or the session broke, not the application.
 *
 * Kept deliberately narrow: anything not on this list is treated as the app
 * rejecting, because misclassifying a real denial as a transport failure would
 * turn a working security control into a red test, and the reverse would let a
 * broken session masquerade as a security guarantee.
 */
const TRANSPORT_SIGNATURES = [
  "invalid session id",
  "session not created",
  "no such window",
  "no such frame",
  "unable to connect",
  "econnrefused",
  "socket hang up",
  "script timeout",
  "asynchronous script timeout",
  "target closed",
];

function classifyThrown<T>(message: string): InvokeOutcome<T> {
  const normalized = message.toLowerCase();
  if (TRANSPORT_SIGNATURES.some((signature) => normalized.includes(signature))) {
    return { status: "transport", message };
  }
  return { status: "rejected", message };
}

/**
 * Invoke a command and return how it settled, without throwing.
 *
 * Security specs need the rejection itself as data, and need to know it came
 * from the app rather than from the driver.
 */
export async function invokeOutcome<T>(
  command: string,
  args: Record<string, unknown> = {},
): Promise<InvokeOutcome<T>> {
  let result: InvokeResult<T>;
  try {
    result = await browser.executeAsync((name, payload, done) => {
      // Serialize inside the page: several commands reject with a structured
      // `{kind, message}` payload, and `String(error)` would flatten that to
      // "[object Object]" before the assertion ever sees it.
      const describe = (error: unknown): string => {
        if (typeof error === "string") return error;
        if (error && typeof error === "object") {
          const structured = error as Record<string, unknown>;
          if (typeof structured.message === "string") {
            return typeof structured.kind === "string"
              ? `${structured.kind}: ${structured.message}`
              : structured.message;
          }
          try {
            return JSON.stringify(error);
          } catch {
            return String(error);
          }
        }
        return String(error);
      };

      const internals = (window as unknown as { __TAURI_INTERNALS__: TauriInternals })
        .__TAURI_INTERNALS__;
      // A denied command can throw synchronously rather than returning a
      // rejected promise, so the guard has to wrap the call itself.
      try {
        internals
          .invoke<T>(name, payload)
          .then((value) => done({ value }))
          .catch((error: unknown) => done({ failure: describe(error) }));
      } catch (error) {
        done({ failure: describe(error) });
      }
    }, command, args);
  } catch (error) {
    // `@wdio/tauri-service` patches `execute`, so an app-level rejection is
    // re-thrown here as a WebDriverError carrying the app's own message rather
    // than arriving through `done({ error })`. That makes this catch the
    // *normal* path for a denied command, not an exceptional one.
    //
    // Classify by signature: only failures that mean the driver or session
    // itself broke count as transport. Everything else is the app talking.
    return classifyThrown(String((error as Error)?.message ?? error));
  }

  if (result.failure !== undefined) {
    return { status: "rejected", message: result.failure };
  }
  return { status: "ok", value: result.value as T };
}

/**
 * Invoke a command and return the rejection message.
 *
 * Throws when the driver failed or when the command unexpectedly succeeded, so
 * neither can be mistaken for a denial.
 */
export async function expectRejection(
  command: string,
  args: Record<string, unknown> = {},
): Promise<string> {
  const outcome = await invokeOutcome<unknown>(command, args);
  if (outcome.status === "transport") {
    throw new Error(
      `invoke ${command} failed in the driver, not the app: ${outcome.message}`,
    );
  }
  if (outcome.status === "ok") {
    throw new Error(`invoke ${command} unexpectedly succeeded; a rejection was required.`);
  }
  return outcome.message;
}

/** Invoke a Tauri command in the webview, throwing on any failure. */
export async function invokeInApp<T>(
  command: string,
  args: Record<string, unknown> = {},
): Promise<T> {
  const outcome = await invokeOutcome<T>(command, args);
  if (outcome.status === "rejected") {
    throw new Error(`invoke ${command} was rejected by the app: ${outcome.message}`);
  }
  if (outcome.status === "transport") {
    throw new Error(`invoke ${command} failed in the driver: ${outcome.message}`);
  }
  return outcome.value;
}
