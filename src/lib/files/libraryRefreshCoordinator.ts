export type LibraryRefreshPriority = "background" | "immediate";

interface RefreshRequestOptions {
  priority?: LibraryRefreshPriority;
  showLoading?: boolean;
}

interface LibraryRefreshCoordinatorConfig {
  isSuppressed: () => boolean;
  isSearchMode: () => boolean;
  refreshRecent: (showLoading: boolean) => Promise<void>;
  refreshSearch: () => Promise<void>;
}

export interface LibraryRefreshCoordinator {
  requestActive: (options?: RefreshRequestOptions) => void;
  requestRecent: (options?: RefreshRequestOptions) => void;
  releaseDeferred: () => void;
  destroy: () => void;
}

/**
 * Serializes sidebar refresh work and coalesces requests that arrive while a
 * fetch is queued or running. Background requests defer while the visible
 * list is frozen; explicit user operations use immediate priority.
 */
export function createLibraryRefreshCoordinator(
  config: LibraryRefreshCoordinatorConfig,
): LibraryRefreshCoordinator {
  let pendingActive = false;
  let pendingRecent = false;
  let pendingRecentLoading = false;
  let deferredActive = false;
  let deferredRecent = false;
  let deferredRecentLoading = false;
  let scheduled = false;
  let running = false;
  let destroyed = false;

  function scheduleFlush(): void {
    if (destroyed || scheduled || running) return;

    scheduled = true;
    queueMicrotask(() => {
      scheduled = false;
      void flush().catch((error: unknown) => {
        console.error("Failed to refresh library sidebar:", error);
      });
    });
  }

  function shouldDefer(priority: LibraryRefreshPriority): boolean {
    return priority === "background" && config.isSuppressed();
  }

  function requestActive(options?: RefreshRequestOptions): void {
    if (destroyed) return;
    if (shouldDefer(options?.priority ?? "background")) {
      deferredActive = true;
      return;
    }

    pendingActive = true;
    pendingRecentLoading ||= options?.showLoading ?? false;
    scheduleFlush();
  }

  function requestRecent(options?: RefreshRequestOptions): void {
    if (destroyed) return;
    if (shouldDefer(options?.priority ?? "background")) {
      deferredRecent = true;
      deferredRecentLoading ||= options?.showLoading ?? false;
      return;
    }

    pendingRecent = true;
    pendingRecentLoading ||= options?.showLoading ?? false;
    scheduleFlush();
  }

  async function flush(): Promise<void> {
    if (destroyed || running) return;
    running = true;

    try {
      while (!destroyed && (pendingActive || pendingRecent)) {
        const refreshActive = pendingActive;
        const refreshRecentExplicitly = pendingRecent;
        const showRecentLoading = pendingRecentLoading;

        pendingActive = false;
        pendingRecent = false;
        pendingRecentLoading = false;

        const activeIsSearch = refreshActive && config.isSearchMode();
        if (refreshRecentExplicitly || (refreshActive && !activeIsSearch)) {
          await config.refreshRecent(showRecentLoading);
        }

        if (!destroyed && activeIsSearch && config.isSearchMode()) {
          await config.refreshSearch();
        }
      }
    } finally {
      running = false;
      if (!destroyed && (pendingActive || pendingRecent)) {
        scheduleFlush();
      }
    }
  }

  function releaseDeferred(): void {
    if (destroyed || (!deferredActive && !deferredRecent)) return;

    pendingActive ||= deferredActive;
    pendingRecent ||= deferredRecent;
    pendingRecentLoading ||= deferredRecentLoading;
    deferredActive = false;
    deferredRecent = false;
    deferredRecentLoading = false;
    scheduleFlush();
  }

  function destroy(): void {
    destroyed = true;
    pendingActive = false;
    pendingRecent = false;
    pendingRecentLoading = false;
    deferredActive = false;
    deferredRecent = false;
    deferredRecentLoading = false;
  }

  return {
    requestActive,
    requestRecent,
    releaseDeferred,
    destroy,
  };
}
