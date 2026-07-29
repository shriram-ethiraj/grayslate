import fs from "node:fs";
import path from "node:path";
import { TIMEOUTS } from "../config/timeouts.js";
import { invokeInApp, type DocumentDescriptor } from "../driver/invoke.js";
import { homeDirectory, notesRoot } from "../helpers/sandbox.js";
import { waitUntilReady } from "../pages/editor.js";

/**
 * Test data provisioning.
 *
 * Every spec seeds its own data. Nothing here reads state another spec left
 * behind — the sandbox is wiped between spec files, so "it was already there"
 * is never a valid assumption.
 */

/** Committed sample files under `e2e/fixtures/`. */
export const fixturesDir = path.resolve(process.cwd(), "e2e", "fixtures");

/** Sandbox location for "external" (non-slate) files, outside the notes root. */
export const externalRoot = path.join(homeDirectory, "external");

export { notesRoot };

/** Copy a committed fixture into the sandbox's external directory. */
export function provisionFixture(name: string, destinationName = name): string {
  fs.mkdirSync(externalRoot, { recursive: true });
  const destination = path.join(externalRoot, destinationName);
  fs.copyFileSync(path.join(fixturesDir, name), destination);
  return destination;
}

/** Write generated text outside the notes root. */
export function provisionText(name: string, content: string): string {
  fs.mkdirSync(externalRoot, { recursive: true });
  const destination = path.join(externalRoot, name);
  fs.writeFileSync(destination, content, "utf8");
  return destination;
}

/** Write exact bytes outside the notes root, for encoding and EOL fixtures. */
export function provisionBytes(name: string, bytes: Buffer): string {
  fs.mkdirSync(externalRoot, { recursive: true });
  const destination = path.join(externalRoot, name);
  fs.writeFileSync(destination, bytes);
  return destination;
}

/** Create a sparse regular file without allocating its logical size on disk. */
export function provisionSparseFile(name: string, byteLength: number): string {
  fs.mkdirSync(externalRoot, { recursive: true });
  const destination = path.join(externalRoot, name);
  const handle = fs.openSync(destination, "w");
  try {
    fs.ftruncateSync(handle, byteLength);
  } finally {
    fs.closeSync(handle);
  }
  return destination;
}

/** Write a file directly into the managed notes root, as a pre-existing slate. */
export function provisionSlate(name: string, content: string | Buffer): string {
  fs.mkdirSync(notesRoot, { recursive: true });
  const destination = path.join(notesRoot, name);
  fs.writeFileSync(destination, content);
  return destination;
}

/** Synthesize a large CSV, for the >100k-row virtualization contract. */
export function writeLargeCsv(filePath: string, rows: number): void {
  fs.mkdirSync(path.dirname(filePath), { recursive: true });
  const handle = fs.openSync(filePath, "w");
  fs.writeSync(handle, "id,name,value\n");
  for (let i = 1; i <= rows; i += 1) {
    fs.writeSync(handle, `${i},row-${i},${i * 2}\n`);
  }
  fs.closeSync(handle);
}

/**
 * Open an existing sandbox path through the real authorized-open path.
 *
 * `e2e_open_path` is compiled only under `--features e2e`. It does not fake the
 * open: it runs the production `pick_document` authorization and grant code and
 * emits the same `files://open-path` event the native dialog would, so
 * everything downstream of the dialog is genuinely exercised. The dialog itself
 * is not drivable by WebDriver — that remains an acknowledged blind spot.
 */
export async function openPath(filePath: string): Promise<DocumentDescriptor> {
  const descriptor = await requestOpenPath(filePath);
  await waitForDocument(descriptor);
  return descriptor;
}

/**
 * Start the authorized open flow but do not wait for the editor to mount it.
 *
 * Cancellation scenarios need to hold the Rust read worker at a deterministic
 * gate, cancel from the UI, and only then release it.
 */
export async function requestOpenPath(filePath: string): Promise<DocumentDescriptor> {
  const descriptor = await invokeInApp<DocumentDescriptor | null>("e2e_open_path", {
    path: filePath,
  });
  if (!descriptor) {
    throw new Error(`Opening '${filePath}' did not return a document grant.`);
  }
  return descriptor;
}

/** Grant a Save-As target path through the real authorization path. */
export async function grantSavePath(
  targetPath: string,
): Promise<DocumentDescriptor | null> {
  return invokeInApp<DocumentDescriptor | null>("e2e_save_path", { path: targetPath });
}

/**
 * Pre-select the answer the next native Open dialog would give.
 *
 * After this, the spec clicks the *real* File → Open item and the production
 * `pick_document` command consumes the queued path instead of blocking on a
 * dialog WebDriver cannot drive. Everything else — classification, granting,
 * the open event, the frontend's open handler — runs unchanged.
 */
export async function queueOpenDialogResult(targetPath: string): Promise<void> {
  await invokeInApp<void>("e2e_queue_open_path", { path: targetPath });
}

/**
 * Pre-select the answer the next native Save As dialog would give.
 */
export async function queueSaveDialogResult(targetPath: string): Promise<void> {
  await invokeInApp<void>("e2e_queue_save_path", { path: targetPath });
}

/**
 * Make the next native Open dialog report that the user cancelled.
 *
 * Queueing *nothing* does not do this. An empty queue is indistinguishable from
 * a test that never intended to open a dialog at all, so the production command
 * falls through to the real native dialog — which, under WebDriver, blocks
 * until the suite times out. Cancellation has to be an explicit answer.
 */
export async function queueOpenDialogCancel(): Promise<void> {
  await invokeInApp<void>("e2e_queue_open_path", { path: null });
}

/** Make the next native Save As dialog report that the user cancelled. */
export async function queueSaveDialogCancel(): Promise<void> {
  await invokeInApp<void>("e2e_queue_save_path", { path: null });
}

/** Provision a committed fixture and open it. */
export async function openFixture(name: string, destinationName = name): Promise<string> {
  const destination = provisionFixture(name, destinationName);
  await openPath(destination);
  return destination;
}

/** Provision generated text and open it. */
export async function openText(name: string, content: string): Promise<string> {
  const destination = provisionText(name, content);
  await openPath(destination);
  return destination;
}

/** Provision exact bytes and open them. */
export async function openBytes(name: string, bytes: Buffer): Promise<string> {
  const destination = provisionBytes(name, bytes);
  await openPath(destination);
  return destination;
}

/**
 * Provision bytes and start opening them, without waiting for the document.
 *
 * For an ambiguous encoding (`legacySingleByte`, `bomlessUtf16`) the read stops
 * and asks the user before decoding, so the document never becomes ready until
 * the caller answers the prompt. Waiting for readiness first would deadlock.
 * The caller drives `dialogs.encodingConfirmation` and then waits itself.
 */
export async function openBytesExpectingPrompt(
  name: string,
  bytes: Buffer,
): Promise<string> {
  const destination = provisionBytes(name, bytes);
  await invokeInApp<DocumentDescriptor | null>("e2e_open_path", { path: destination });
  return destination;
}

/**
 * Decode a file the way the Rust reader does, so a length comparison is valid.
 *
 * Reading everything as UTF-8 is wrong for the encodings this app explicitly
 * supports: a UTF-16LE fixture decoded as UTF-8 yields roughly double the
 * characters plus replacement characters, and a UTF-8 BOM contributes three
 * bytes that never reach the editor. `read_file_content`
 * (`src-tauri/src/commands/file.rs`) strips the BOM and decodes UTF-16, so the
 * test must model the same thing or every encoding fixture mismatches.
 */
function decodeLikeBackend(filePath: string): string {
  const bytes = fs.readFileSync(filePath);

  if (bytes.length >= 3 && bytes[0] === 0xef && bytes[1] === 0xbb && bytes[2] === 0xbf) {
    return bytes.subarray(3).toString("utf8");
  }
  if (bytes.length >= 2 && bytes[0] === 0xff && bytes[1] === 0xfe) {
    return bytes.subarray(2).toString("utf16le");
  }
  if (bytes.length >= 2 && bytes[0] === 0xfe && bytes[1] === 0xff) {
    // Node has no utf16be decoder; swap the byte pairs and reuse utf16le.
    const swapped = Buffer.from(bytes.subarray(2));
    swapped.swap16();
    return swapped.toString("utf16le");
  }
  return bytes.toString("utf8");
}

/**
 * Wait for the frontend to finish mounting a granted document.
 *
 * CodeMirror holds documents canonically LF-terminated, so a CRLF file on disk
 * reports one character fewer per line. Normalize before comparing or this
 * never matches a Windows-style fixture.
 */
export async function waitForDocument(descriptor: DocumentDescriptor): Promise<void> {
  const documentLength = decodeLikeBackend(descriptor.displayPath).replace(
    /\r\n?/g,
    "\n",
  ).length;
  await waitUntilReady({
    documentPath: descriptor.displayPath,
    documentLength,
    timeoutMs: TIMEOUTS.editor,
  });
}
