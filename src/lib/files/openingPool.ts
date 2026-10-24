/** Each batch owns four slots, including a load in its receiving window. */
const OPENING_CONCURRENCY = 4;

/**
 * Keep results in task order while refilling each slot as soon as it settles.
 * A slot includes the entire operation (window readiness and encoding prompts),
 * not just window creation. Rejections and synchronous throws are independent.
 */
export async function settleOpeningTasks<T>(
  tasks: readonly (() => Promise<T>)[],
): Promise<PromiseSettledResult<T>[]> {
  const results: PromiseSettledResult<T>[] = new Array(tasks.length);
  let next = 0;
  async function worker(): Promise<void> {
    while (next < tasks.length) {
      const index = next++;
      try {
        results[index] = { status: "fulfilled", value: await tasks[index]() };
      } catch (reason: unknown) {
        results[index] = { status: "rejected", reason };
      }
    }
  }
  // Only the workers run through Promise.all; the request itself stays bounded.
  await Promise.all(Array.from({ length: Math.min(OPENING_CONCURRENCY, tasks.length) }, worker));
  return results;
}
