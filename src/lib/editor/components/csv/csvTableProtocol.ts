export type CsvTableSnapshot = {
    headers: string[];
    rowCount: number;
    delimiter: string;
    errors: string[];
    version: number;
    liveMirrorEnabled: boolean;
};

export type CsvRowWindow = {
    start: number;
    rows: string[][];
    version: number;
};

export type CsvSelectionBlock = {
    startRow: number;
    endRow: number;
    startCol: number;
    endCol: number;
} | null;

export const LIVE_MIRROR_ROW_THRESHOLD = 100_000;

export type CsvMirrorTextUpdate = {
    text: string;
    userEvent: string;
    version: number;
};

export type CsvTableFlushResult = {
    text: string;
    version: number;
};

export type CsvMutationRequest =
    | {
          type: "edit-cell";
          rowIndex: number;
          colIndex: number;
          newValue: string;
      }
    | {
          type: "edit-header";
          colIndex: number;
          newValue: string;
      }
    | {
          type: "clear-cell";
          rowIndex: number;
          colIndex: number;
      }
    | {
          type: "clear-selection";
          startRow: number;
          endRow: number;
          startCol: number;
          endCol: number;
      }
    | {
          type: "delete-rows";
          start: number;
          end: number;
      }
    | {
          type: "delete-columns";
          start: number;
          end: number;
      }
    | {
          type: "add-row";
          index: number;
      }
    | {
          type: "add-column";
          index: number;
      }
    | {
          type: "move-rows";
          start: number;
          end: number;
          direction: -1 | 1;
      }
    | {
          type: "move-columns";
          start: number;
          end: number;
          direction: -1 | 1;
      }
    | {
          type: "duplicate-rows";
          start: number;
          end: number;
      };

/**
 * Response from Rust csv_mutate / csv_undo / csv_redo commands.
 * The mirror text is inlined in the response (no fire-and-forget channel).
 */
export type CsvMutationResponse = {
    snapshot: CsvTableSnapshot;
    applied: boolean;
    mirrorText: string | null;
    mirrorUserEvent: string | null;
};

export interface CsvTableController {
    getSnapshot(): CsvTableSnapshot;
    getCachedCellValue(rowIndex: number, colIndex: number): string;
    fetchCellValue(rowIndex: number, colIndex: number): Promise<string>;
    runMutation(mutation: CsvMutationRequest, userEvent: string): Promise<boolean>;
    undo(): Promise<boolean>;
    redo(): Promise<boolean>;
}