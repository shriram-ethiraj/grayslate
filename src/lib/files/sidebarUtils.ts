/**
 * Pure utility types, constants, formatters, and comparators for the Library sidebar.
 * Nothing in this file depends on Svelte state or component lifecycle.
 */

import type { RecentFileRecord, RecentFileSource, SidebarSearchResult } from "$lib/files/recentFiles";

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export type FilterMode = "unified" | RecentFileSource;

export type SortMode =
    | "recently-opened"
    | "least-recently-opened"
    | "name-asc"
    | "name-desc"
    | "size-desc"
    | "size-asc";

export type RecencyBucket = "today" | "this-week" | "older";

/** Union of the two record shapes that can appear in the sidebar list. */
export type LibraryFileRecord = RecentFileRecord | SidebarSearchResult;

export interface RecentFileSection {
    key: RecencyBucket | "all";
    label: string;
    items: LibraryFileRecord[];
}

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

export const RECENT_FILES_LIMIT = 120;
export const DEFAULT_FILTER_MODE: FilterMode = "unified";
export const DEFAULT_SORT_MODE: SortMode = "recently-opened";

/** Display order of recency buckets for each recency sort direction. */
export const recencySectionOrder: Record<
    Extract<SortMode, "recently-opened" | "least-recently-opened">,
    RecencyBucket[]
> = {
    "recently-opened": ["today", "this-week", "older"],
    "least-recently-opened": ["older", "this-week", "today"],
};

export const recencySectionLabels: Record<RecencyBucket, string> = {
    today: "Today",
    "this-week": "This week",
    older: "Older",
};

// ---------------------------------------------------------------------------
// Formatting
// ---------------------------------------------------------------------------

const shortDayFormatter = new Intl.DateTimeFormat(undefined, { weekday: "short" });
const shortDateFormatter = new Intl.DateTimeFormat(undefined, { month: "short", day: "numeric" });
const fullDateFormatter = new Intl.DateTimeFormat(undefined, { day: "numeric", month: "short", year: "numeric" });
const fullTimestampFormatter = new Intl.DateTimeFormat(undefined, {
    weekday: "long", day: "numeric", month: "long", year: "numeric",
    hour: "numeric", minute: "2-digit",
});

/**
 * Formats a Unix-ms timestamp as a compact, human-readable string (Gmail/Notion style).
 * Returns "Unknown" when the value is null/zero.
 *
 * < 1 min  → "Just now"
 * < 1 hr   → "5 min ago"
 * < 24 hrs → "3h ago"
 * < 48 hrs → "Yesterday"
 * < 7 days → "Mon" / "Tue" (short day name)
 * same yr  → "Mar 14"
 * older    → "14 Mar 2024"
 */
export function formatTimestamp(value: number | null): string {
    if (!value) {
        return "Unknown";
    }

    const now = Date.now();
    const diffSec = Math.floor((now - value) / 1_000);

    if (diffSec < 60) {
        return "Just now";
    }

    if (diffSec < 3_600) {
        return `${Math.floor(diffSec / 60)} min ago`;
    }

    if (diffSec < 86_400) {
        return `${Math.floor(diffSec / 3_600)}h ago`;
    }

    if (diffSec < 172_800) {
        return "Yesterday";
    }

    const date = new Date(value);
    const nowDate = new Date(now);

    if (diffSec < 604_800) {
        return shortDayFormatter.format(date);
    }

    if (date.getFullYear() === nowDate.getFullYear()) {
        return shortDateFormatter.format(date);
    }

    return fullDateFormatter.format(date);
}

/**
 * Returns the full absolute timestamp string suitable for use as a tooltip,
 * e.g. "Friday, 14 March 2026, 11:42 AM".
 */
export function formatTimestampFull(value: number | null): string {
    if (!value) return "";
    return fullTimestampFormatter.format(new Date(value));
}

/**
 * Formats a byte count as a human-readable size string, e.g. "4.2 MB".
 * Returns an empty string for null/zero values.
 */
export function formatSize(value: number | null): string {
    if (!value || value <= 0) {
        return "";
    }

    const units = ["B", "KB", "MB", "GB"];
    let size = value;
    let unitIndex = 0;

    while (size >= 1024 && unitIndex < units.length - 1) {
        size /= 1024;
        unitIndex += 1;
    }

    const rounded = size >= 10 || unitIndex === 0 ? Math.round(size) : Number(size.toFixed(1));
    return `${rounded} ${units[unitIndex]}`;
}

// ---------------------------------------------------------------------------
// Recency helpers
// ---------------------------------------------------------------------------

/**
 * Returns the most relevant recency timestamp for a record.
 * Priority: last_opened_at → last_saved_at → last_seen_at.
 */
export function getRecencyTimestamp(recentFile: LibraryFileRecord): number | null {
    return recentFile.last_opened_at
        ?? recentFile.last_saved_at
        ?? recentFile.last_seen_at;
}

/**
 * Buckets a timestamp into "today", "this-week", or "older"
 * relative to the start of the current local day.
 */
export function getRecencyBucket(timestamp: number | null): RecencyBucket {
    if (!timestamp) {
        return "older";
    }

    const startOfToday = new Date();
    startOfToday.setHours(0, 0, 0, 0);

    if (timestamp >= startOfToday.getTime()) {
        return "today";
    }

    // "This week" = the 6 days before today.
    const startOfThisWeek = startOfToday.getTime() - (6 * 24 * 60 * 60 * 1000);
    return timestamp >= startOfThisWeek ? "this-week" : "older";
}

/**
 * Groups a sorted flat list of files into labelled recency sections.
 * Empty sections are omitted. Section order follows `recencySectionOrder`.
 */
export function buildRecencySections(
    files: LibraryFileRecord[],
    sortOrder: Extract<SortMode, "recently-opened" | "least-recently-opened">,
): RecentFileSection[] {
    const sectionItems: Record<RecencyBucket, LibraryFileRecord[]> = {
        today: [],
        "this-week": [],
        older: [],
    };

    for (const recentFile of files) {
        sectionItems[getRecencyBucket(getRecencyTimestamp(recentFile))].push(recentFile);
    }

    return recencySectionOrder[sortOrder]
        .map((bucket) => ({
            key: bucket,
            label: recencySectionLabels[bucket],
            items: sectionItems[bucket],
        }))
        .filter((section) => section.items.length > 0);
}

// ---------------------------------------------------------------------------
// Sort comparators
// ---------------------------------------------------------------------------

const textCollator = new Intl.Collator(undefined, {
    numeric: true,
    sensitivity: "base",
});

/** Null-safe numeric comparator. Nulls sort to the end. */
export function compareNumbers(left: number | null, right: number | null): number {
    if (left === right) return 0;
    if (left === null) return 1;
    if (right === null) return -1;
    return left - right;
}

/** Null-safe locale-aware text comparator. Nulls sort to the end. */
export function compareText(
    left: string | null | undefined,
    right: string | null | undefined,
): number {
    if (left === right) return 0;
    if (!left) return 1;
    if (!right) return -1;
    return textCollator.compare(left, right);
}

/**
 * Compares two recent-file records by the given sort order.
 * Falls back to name → path as a stable tiebreaker.
 */
export function compareRecentFiles(
    left: RecentFileRecord,
    right: RecentFileRecord,
    sortOrder: SortMode,
): number {
    switch (sortOrder) {
        case "recently-opened": {
            const byTimestamp = compareNumbers(getRecencyTimestamp(right), getRecencyTimestamp(left));
            if (byTimestamp !== 0) return byTimestamp;
            break;
        }
        case "least-recently-opened": {
            const byTimestamp = compareNumbers(getRecencyTimestamp(left), getRecencyTimestamp(right));
            if (byTimestamp !== 0) return byTimestamp;
            break;
        }
        case "name-asc": {
            const byName = compareText(left.file_name, right.file_name);
            if (byName !== 0) return byName;
            break;
        }
        case "name-desc": {
            const byName = compareText(right.file_name, left.file_name);
            if (byName !== 0) return byName;
            break;
        }
        case "size-desc": {
            const bySize = compareNumbers(right.size_bytes, left.size_bytes);
            if (bySize !== 0) return bySize;
            break;
        }
        case "size-asc": {
            const bySize = compareNumbers(left.size_bytes, right.size_bytes);
            if (bySize !== 0) return bySize;
            break;
        }
    }

    // Stable tiebreaker: name then full path.
    const byName = compareText(left.file_name, right.file_name);
    if (byName !== 0) return byName;
    return compareText(left.path, right.path);
}

/**
 * Compares two search results. Final score always dominates;
 * `sortOrder` is used only as a tiebreaker via `compareRecentFiles`.
 */
export function compareSearchResults(
    left: SidebarSearchResult,
    right: SidebarSearchResult,
    sortOrder: SortMode,
): number {
    const byScore = right.final_score - left.final_score;
    if (byScore !== 0) return byScore;
    return compareRecentFiles(left, right, sortOrder);
}

// ---------------------------------------------------------------------------
// Search highlight helpers
// ---------------------------------------------------------------------------

export interface HighlightFragment {
    text: string;
    isMatch: boolean;
}

/**
 * Splits `text` into alternating matched/unmatched fragments based on the
 * given search `terms`. Matching is case-insensitive. Longer terms are
 * matched first to avoid partial-overlap ambiguity.
 *
 * This mirrors the backend tokenization behavior: whitespace-split,
 * lowercased terms matched as literal substrings.
 */
export function splitTextByTerms(
    text: string,
    terms: string[],
): HighlightFragment[] {
    if (!text || terms.length === 0) {
        return [{ text, isMatch: false }];
    }

    const escaped = terms
        .filter((t) => t.length > 0)
        .map((t) => t.replace(/[.*+?^${}()|[\]\\]/g, "\\$&"))
        .sort((a, b) => b.length - a.length);

    if (escaped.length === 0) {
        return [{ text, isMatch: false }];
    }

    const pattern = new RegExp(`(${escaped.join("|")})`, "gi");
    const parts: HighlightFragment[] = [];
    let lastIndex = 0;
    let match: RegExpExecArray | null;

    while ((match = pattern.exec(text)) !== null) {
        if (match.index > lastIndex) {
            parts.push({ text: text.slice(lastIndex, match.index), isMatch: false });
        }
        parts.push({ text: match[0], isMatch: true });
        lastIndex = match.index + match[0].length;
    }

    if (lastIndex < text.length) {
        parts.push({ text: text.slice(lastIndex), isMatch: false });
    }

    return parts.length > 0 ? parts : [{ text, isMatch: false }];
}

/** Returns true when the record is a `SidebarSearchResult` (search mode). */
export function isSearchResult(record: LibraryFileRecord): record is SidebarSearchResult {
    return "match_count" in record;
}

/**
 * Returns a short excerpt of `lineText` centred around the first query match.
 *
 * If the match is near the start the excerpt begins there; if it is deep into
 * a long line we skip ahead so the highlighted text appears near the left edge
 * rather than being hidden off-screen. Leading/trailing truncation is indicated
 * with a `…` character.
 *
 * @param lineText      Full line text delivered by the backend.
 * @param terms         Active search terms used to locate the earliest match.
 * @param contextBefore Characters to preserve before the match start (default 20).
 * @param maxLength     Maximum excerpt length before truncation (default 80).
 */
export function getLineExcerpt(
    lineText: string,
    terms: string[],
    contextBefore = 20,
    maxLength = 80,
): string {
    const trimmed = lineText.trim();

    if (trimmed.length <= maxLength || terms.length === 0) {
        return trimmed.length > maxLength ? trimmed.slice(0, maxLength) + "…" : trimmed;
    }

    // Find the earliest match position across all terms.
    const lower = trimmed.toLowerCase();
    let matchStart = -1;
    for (const term of terms) {
        if (!term) continue;
        const idx = lower.indexOf(term.toLowerCase());
        if (idx !== -1 && (matchStart === -1 || idx < matchStart)) {
            matchStart = idx;
        }
    }

    // No term found in the line — show from the beginning.
    if (matchStart === -1) {
        return trimmed.slice(0, maxLength) + "…";
    }

    // Anchor the window so the match is `contextBefore` chars from the left edge.
    const start = Math.max(0, matchStart - contextBefore);
    const end = Math.min(trimmed.length, start + maxLength);

    let excerpt = trimmed.slice(start, end);
    if (start > 0) excerpt = "…" + excerpt;
    if (end < trimmed.length) excerpt += "…";

    return excerpt;
}
