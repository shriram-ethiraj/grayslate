export type CsvTableSnapshot = {
    headers: string[];
    rowCount: number;
    delimiter: string;
    errors: string[];
    version: number;
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

export type CsvReplayStep = {
  text: string;
  userEvent: string;
};

export type CsvTableFlushResult = {
  baseText: string;
  text: string;
  replaySteps: CsvReplayStep[];
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
      };

export type CsvWorkerRequest =
    | {
          type: "initialize";
          requestId: number;
          text: string;
      }
    | {
          type: "get-rows";
          requestId: number;
          start: number;
          end: number;
      }
    | {
          type: "get-cell";
          requestId: number;
          rowIndex: number;
          colIndex: number;
      }
    | {
          type: "mutate";
          requestId: number;
          mutation: CsvMutationRequest;
      }
    | {
          type: "undo";
          requestId: number;
      }
    | {
          type: "redo";
          requestId: number;
      }
        | {
          type: "flush-text";
          requestId: number;
      };

export type CsvWorkerResponse =
    | {
          type: "initialize-progress";
          requestId: number;
          parsedRows: number;
      }
    | {
          type: "initialized";
          requestId: number;
          snapshot: CsvTableSnapshot;
      }
    | {
          type: "rows";
          requestId: number;
          window: CsvRowWindow;
      }
    | {
          type: "cell";
          requestId: number;
          value: string;
      }
    | {
          type: "mutation-applied";
          requestId: number;
          snapshot: CsvTableSnapshot;
          text: string;
          applied: boolean;
      }
        | {
          type: "flushed-text";
          requestId: number;
          text: string;
          version: number;
      }
    | {
          type: "error";
          requestId: number;
          error: string;
      };

export interface CsvTableController {
    getSnapshot(): CsvTableSnapshot;
    getCachedCellValue(rowIndex: number, colIndex: number): string;
    fetchCellValue(rowIndex: number, colIndex: number): Promise<string>;
    runMutation(mutation: CsvMutationRequest, userEvent: string): Promise<boolean>;
    undo(): Promise<boolean>;
    redo(): Promise<boolean>;
}