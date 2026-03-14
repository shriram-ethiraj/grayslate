export type FindStatsWorkerRequest =
    | {
        type: "scan";
        requestId: number;
        text: string;
        search: string;
        caseSensitive: boolean;
        literal: boolean;
        regexp: boolean;
        wholeWord: boolean;
        selectionFrom: number;
        selectionTo: number;
    }
    | {
        type: "selection";
        requestId: number;
        selectionFrom: number;
        selectionTo: number;
    };

export type FindStatsWorkerResponse =
    | {
        type: "result";
        requestId: number;
        matchCount: number;
        currentMatch: number;
        approximate: boolean;
    }
    | {
        type: "error";
        requestId: number;
        error: string;
    };
