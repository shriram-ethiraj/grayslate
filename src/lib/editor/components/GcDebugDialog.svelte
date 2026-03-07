<script lang="ts">
  import { onDestroy } from "svelte";
  import * as Dialog from "$lib/components/ui/dialog/index.js";
  import { gcDebugControls } from "$lib/state/gc-debug-controls.svelte";
  import { gcDebugState } from "$lib/state/gc-debug.svelte";
  import { reclaimMemory } from "$lib/editor/core/memory";

  let now = $state(Date.now());
  let timer: ReturnType<typeof setInterval> | undefined;

  $effect(() => {
    if (!gcDebugControls.panelOpen) {
      if (timer !== undefined) {
        clearInterval(timer);
        timer = undefined;
      }
      return;
    }

    now = Date.now();
    timer = setInterval(() => {
      now = Date.now();
    }, 1000);

    return () => {
      if (timer !== undefined) {
        clearInterval(timer);
        timer = undefined;
      }
    };
  });

  onDestroy(() => {
    if (timer !== undefined) {
      clearInterval(timer);
    }
  });

  function formatBytes(bytes: number): string {
    if (bytes <= 0) return "0 B";

    const units = ["B", "KiB", "MiB", "GiB"];
    let value = bytes;
    let unitIndex = 0;

    while (value >= 1024 && unitIndex < units.length - 1) {
      value /= 1024;
      unitIndex += 1;
    }

    const digits = value >= 10 || unitIndex === 0 ? 0 : 1;
    return `${value.toFixed(digits)} ${units[unitIndex]}`;
  }

  function formatMs(ms: number): string {
    if (ms <= 0) return "0 ms";
    if (ms >= 60_000)
      return `${(ms / 60_000).toFixed(ms >= 600_000 ? 0 : 1)} min`;
    if (ms >= 1000) return `${(ms / 1000).toFixed(ms >= 10_000 ? 0 : 1)} s`;
    return `${Math.round(ms)} ms`;
  }

  function formatRatio(value: number): string {
    return `${(value * 100).toFixed(1)}%`;
  }

  function formatTimestamp(timestamp: number): string {
    if (timestamp <= 0) return "Never";
    return new Intl.DateTimeFormat(undefined, {
      hour: "2-digit",
      minute: "2-digit",
      second: "2-digit",
    }).format(timestamp);
  }

  function formatAge(timestamp: number): string {
    if (timestamp <= 0) return "";

    const ageMs = Math.max(0, now - timestamp);
    if (ageMs < 4000) return "now";
    if (ageMs < 60_000) return `${Math.round(ageMs / 1000)}s ago`;
    if (ageMs < 3_600_000) return `${Math.round(ageMs / 60_000)}m ago`;
    return `${Math.round(ageMs / 3_600_000)}h ago`;
  }

  function getStageBadgeClass(stage: string): string {
    switch (stage) {
      case "completed":
        return "bg-emerald-500/12 text-emerald-700 dark:text-emerald-300";
      case "failed":
        return "bg-rose-500/12 text-rose-700 dark:text-rose-300";
      case "running":
        return "bg-sky-500/12 text-sky-700 dark:text-sky-300";
      case "skipped":
        return "bg-amber-500/12 text-amber-700 dark:text-amber-300";
      default:
        return "bg-muted text-foreground/80";
    }
  }

  function getStageCardClass(stage: string): string {
    switch (stage) {
      case "completed":
        return "border-emerald-500/30 bg-emerald-500/6";
      case "failed":
        return "border-rose-500/30 bg-rose-500/6";
      case "running":
        return "border-sky-500/30 bg-sky-500/6";
      case "skipped":
        return "border-amber-500/30 bg-amber-500/6";
      default:
        return "border-border bg-muted/20";
    }
  }
</script>

<Dialog.Root bind:open={gcDebugControls.panelOpen}>
  <Dialog.Content
    class="h-[min(80vh,820px)] w-[min(980px,calc(100vw-2rem))] overflow-hidden p-0"
  >
    <div class="absolute inset-0 flex flex-col">
      <Dialog.Header class="border-b px-6 pt-6 pb-4">
        <div class="flex items-start justify-between">
          <div>
            <Dialog.Title>GC Diagnostics</Dialog.Title>
            <Dialog.Description class="mt-1.5">
              Live reclaim state, thresholds, and event history.
            </Dialog.Description>
          </div>
          <button
            class="inline-flex h-8 items-center justify-center rounded-md bg-secondary px-3 text-xs font-medium text-secondary-foreground shadow-sm hover:bg-secondary/80 focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring"
            onclick={() => reclaimMemory()}
          >
            Force trigger
          </button>
        </div>
      </Dialog.Header>

      <div class="grid min-h-0 flex-1 gap-0 md:grid-cols-[360px_minmax(0,1fr)]">
        <div
          class="min-h-0 overflow-y-auto border-b p-6 md:border-r md:border-b-0"
        >
          <div class="space-y-5 text-sm">
            <section class="space-y-2">
              <h3 class="font-semibold text-foreground">Current decision</h3>
              <div
                class={`rounded-md border p-3 text-xs leading-5 ${getStageCardClass(gcDebugState.snapshot.lastDecisionStage)}`}
              >
                <div class="font-medium text-foreground">
                  <span
                    class={`inline-flex rounded px-1.5 py-0.5 text-[10px] uppercase tracking-wide ${getStageBadgeClass(gcDebugState.snapshot.lastDecisionStage)}`}
                  >
                    {gcDebugState.snapshot.lastDecisionStage}
                  </span>
                </div>
                <div class="mt-1">
                  {gcDebugState.snapshot.lastDecisionReason}
                </div>
                {#if gcDebugState.snapshot.lastError}
                  <div class="mt-2 text-[hsl(0,80%,60%)]">
                    Error: {gcDebugState.snapshot.lastError}
                  </div>
                {/if}
              </div>
            </section>

            <section class="space-y-2 text-xs leading-5">
              <h3 class="font-semibold text-foreground">State</h3>
              <div class="grid grid-cols-[auto_1fr] gap-x-3 gap-y-1">
                <span class="text-foreground/70">Last source</span>
                <span>{gcDebugState.snapshot.lastTriggerSource}</span>
                <span class="text-foreground/70">Last run</span>
                <span>
                  {formatTimestamp(gcDebugState.snapshot.lastReclaimAt)}
                  {#if gcDebugState.snapshot.lastReclaimAt > 0}
                    <span class="text-foreground/60">
                      ({formatAge(gcDebugState.snapshot.lastReclaimAt)})
                    </span>
                  {/if}
                </span>
              </div>
            </section>

            <section class="space-y-2 text-xs leading-5">
              <h3 class="font-semibold text-foreground">Document signals</h3>
              <div class="grid grid-cols-[auto_1fr] gap-x-3 gap-y-1">
                <span class="text-foreground/70">Current doc</span>
                <span>{formatBytes(gcDebugState.snapshot.currentDocBytes)}</span
                >
                <span class="text-foreground/70">Last shrink</span>
                <span>{formatBytes(gcDebugState.snapshot.lastShrinkBytes)}</span
                >
                <span class="text-foreground/70">Shrink ratio</span>
                <span>{formatRatio(gcDebugState.snapshot.lastShrinkRatio)}</span
                >
                <span class="text-foreground/70">Post-shrink ratio</span>
                <span
                  >{formatRatio(
                    gcDebugState.snapshot.lastPostShrinkRatio,
                  )}</span
                >
              </div>
            </section>

            <section class="space-y-2 text-xs leading-5">
              <h3 class="font-semibold text-foreground">Memory telemetry</h3>
              <div class="grid grid-cols-[auto_1fr] gap-x-3 gap-y-1">
                <span class="text-foreground/70">System used</span>
                <span>{formatBytes(gcDebugState.snapshot.lastSystemUsed)}</span>
                <span class="text-foreground/70">Available RAM</span>
                <span>{formatBytes(gcDebugState.snapshot.lastAvailable)}</span>
                <span class="text-foreground/70">Pressure</span>
                <span
                  >{formatBytes(gcDebugState.snapshot.lastPressureBytes)}</span
                >
              </div>
            </section>

            <section class="space-y-2 text-xs leading-5">
              <h3 class="font-semibold text-foreground">Thresholds</h3>
              <div class="grid grid-cols-[auto_1fr] gap-x-3 gap-y-1">
                <span class="text-foreground/70">Min available RAM</span>
                <span
                  >{formatBytes(
                    gcDebugState.snapshot.minAvailableRamBytes,
                  )}</span
                >
                <span class="text-foreground/70">Min peak</span>
                <span>{formatBytes(gcDebugState.snapshot.minPeakDocBytes)}</span
                >
                <span class="text-foreground/70">Min shrink</span>
                <span>{formatBytes(gcDebugState.snapshot.minShrinkBytes)}</span>
                <span class="text-foreground/70">Min shrink ratio</span>
                <span>{formatRatio(gcDebugState.snapshot.minShrinkRatio)}</span>
                <span class="text-foreground/70">Max post ratio</span>
                <span
                  >{formatRatio(gcDebugState.snapshot.maxPostShrinkRatio)}</span
                >
              </div>
            </section>
          </div>
        </div>

        <div class="flex min-h-0 flex-col">
          <div class="border-b px-6 py-4">
            <h3 class="font-semibold text-foreground">Event log</h3>
            <p class="mt-1 text-xs text-muted-foreground">
              Newest first. Entries marked "now" are recent live activity.
            </p>
          </div>

          <div class="min-h-0 flex-1 overflow-y-auto px-6 py-4">
            {#if gcDebugState.logs.length === 0}
              <div
                class="rounded-md border border-dashed p-4 text-sm text-muted-foreground"
              >
                No GC events recorded yet.
              </div>
            {:else}
              <div class="space-y-3">
                {#each gcDebugState.logs as entry (entry.id)}
                  <div
                    class={`rounded-md border p-3 text-xs leading-5 ${getStageCardClass(entry.stage)}`}
                  >
                    <div class="flex flex-wrap items-center gap-x-2 gap-y-1">
                      <span class="font-medium text-foreground"
                        >{formatTimestamp(entry.timestamp)}</span
                      >
                      <span class="text-muted-foreground"
                        >{formatAge(entry.timestamp)}</span
                      >
                      <span
                        class={`rounded px-1.5 py-0.5 text-[10px] uppercase tracking-wide ${getStageBadgeClass(entry.stage)}`}
                      >
                        {entry.stage}
                      </span>
                      {#if entry.source !== "none"}
                        <span
                          class="rounded bg-muted px-1.5 py-0.5 text-[10px] uppercase tracking-wide text-foreground/80"
                        >
                          {entry.source}
                        </span>
                      {/if}
                    </div>
                    <div class="mt-2">{entry.message}</div>
                  </div>
                {/each}
              </div>
            {/if}
          </div>
        </div>
      </div>
    </div>
  </Dialog.Content>
</Dialog.Root>
