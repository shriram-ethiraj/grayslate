use serde::Deserialize;

use super::ops::TableOp;

// ---------------------------------------------------------------------------
// Mutation requests (matches the frontend CsvMutationRequest)
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum CsvMutationRequest {
    #[serde(rename_all = "camelCase")]
    EditCell {
        row_index: usize,
        col_index: usize,
        new_value: String,
    },
    #[serde(rename_all = "camelCase")]
    EditHeader {
        col_index: usize,
        new_value: String,
    },
    #[serde(rename_all = "camelCase")]
    ClearCell {
        row_index: usize,
        col_index: usize,
    },
    #[serde(rename_all = "camelCase")]
    ClearSelection {
        start_row: usize,
        end_row: usize,
        start_col: usize,
        end_col: usize,
    },
    DeleteRows {
        start: usize,
        end: usize,
    },
    DeleteColumns {
        start: usize,
        end: usize,
    },
    AddRow {
        index: usize,
    },
    AddColumn {
        index: usize,
    },
    MoveRows {
        start: usize,
        end: usize,
        direction: i32,
    },
    MoveColumns {
        start: usize,
        end: usize,
        direction: i32,
    },
}

pub struct MutationResult {
    pub ops: Vec<TableOp>,
    pub applied: bool,
}

/// Build the `TableOp` list for a given mutation request. Does NOT apply
/// or commit — the caller decides whether to commit.
pub fn build_mutation_ops(
    headers: &[String],
    rows: &[Vec<String>],
    mutation: &CsvMutationRequest,
) -> MutationResult {
    match mutation {
        CsvMutationRequest::EditCell {
            row_index,
            col_index,
            new_value,
        } => {
            let old_value = rows
                .get(*row_index)
                .and_then(|r| r.get(*col_index))
                .cloned()
                .unwrap_or_default();
            if old_value == *new_value {
                return MutationResult {
                    ops: vec![],
                    applied: false,
                };
            }
            MutationResult {
                ops: vec![TableOp::Cell {
                    row: *row_index,
                    col: *col_index,
                    old_value,
                    new_value: new_value.clone(),
                }],
                applied: true,
            }
        }
        CsvMutationRequest::EditHeader {
            col_index,
            new_value,
        } => {
            let old_value = headers.get(*col_index).cloned().unwrap_or_default();
            if old_value == *new_value {
                return MutationResult {
                    ops: vec![],
                    applied: false,
                };
            }
            MutationResult {
                ops: vec![TableOp::HeaderCell {
                    col: *col_index,
                    old_value,
                    new_value: new_value.clone(),
                }],
                applied: true,
            }
        }
        CsvMutationRequest::ClearCell {
            row_index,
            col_index,
        } => {
            let old_value = rows
                .get(*row_index)
                .and_then(|r| r.get(*col_index))
                .cloned()
                .unwrap_or_default();
            if old_value.is_empty() {
                return MutationResult {
                    ops: vec![],
                    applied: false,
                };
            }
            MutationResult {
                ops: vec![TableOp::Cell {
                    row: *row_index,
                    col: *col_index,
                    old_value,
                    new_value: String::new(),
                }],
                applied: true,
            }
        }
        CsvMutationRequest::ClearSelection {
            start_row,
            end_row,
            start_col,
            end_col,
        } => {
            let mut old_values = Vec::new();
            let mut has_changes = false;
            for ri in *start_row..=*end_row {
                let mut old_row = Vec::new();
                for ci in *start_col..=*end_col {
                    let value = rows
                        .get(ri)
                        .and_then(|r| r.get(ci))
                        .cloned()
                        .unwrap_or_default();
                    if !value.is_empty() {
                        has_changes = true;
                    }
                    old_row.push(value);
                }
                old_values.push(old_row);
            }
            if !has_changes {
                return MutationResult {
                    ops: vec![],
                    applied: false,
                };
            }
            MutationResult {
                ops: vec![TableOp::BulkCellClear {
                    start_row: *start_row,
                    end_row: *end_row,
                    start_col: *start_col,
                    end_col: *end_col,
                    old_values,
                }],
                applied: true,
            }
        }
        CsvMutationRequest::DeleteRows { start, end } => {
            let s = (*start).min(rows.len());
            let e = (*end + 1).min(rows.len());
            if s >= e {
                return MutationResult {
                    ops: vec![],
                    applied: false,
                };
            }
            let deleted = rows[s..e].to_vec();
            MutationResult {
                ops: vec![TableOp::BulkRowDelete {
                    start: s,
                    end: e - 1,
                    data: deleted,
                }],
                applied: true,
            }
        }
        CsvMutationRequest::DeleteColumns { start, end } => {
            let s = *start;
            let e = *end;
            if s >= headers.len() {
                return MutationResult {
                    ops: vec![],
                    applied: false,
                };
            }
            let actual_end = e.min(headers.len() - 1);
            let deleted_headers: Vec<String> = headers[s..=actual_end].to_vec();
            let deleted_data: Vec<Vec<String>> = rows
                .iter()
                .map(|row| {
                    let end_idx = (actual_end + 1).min(row.len());
                    if s < row.len() {
                        row[s..end_idx].to_vec()
                    } else {
                        Vec::new()
                    }
                })
                .collect();
            MutationResult {
                ops: vec![TableOp::BulkColDelete {
                    start: s,
                    end: actual_end,
                    headers: deleted_headers,
                    data: deleted_data,
                }],
                applied: true,
            }
        }
        CsvMutationRequest::AddRow { index } => {
            let width = headers.len().max(1);
            MutationResult {
                ops: vec![TableOp::RowAdd {
                    index: *index,
                    data: vec![String::new(); width],
                }],
                applied: true,
            }
        }
        CsvMutationRequest::AddColumn { index } => {
            let row_count = rows.len().max(1);
            MutationResult {
                ops: vec![TableOp::BulkColAdd {
                    start: *index,
                    headers: vec![String::new()],
                    data: vec![vec![String::new()]; row_count],
                }],
                applied: true,
            }
        }
        CsvMutationRequest::MoveRows {
            start,
            end,
            direction,
        } => {
            let count = end.saturating_sub(*start) + 1;
            let target_start = if *direction < 0 {
                if *start == 0 {
                    return MutationResult {
                        ops: vec![],
                        applied: false,
                    };
                }
                *start - 1
            } else {
                let t = *start + 1;
                if t + count > rows.len() {
                    return MutationResult {
                        ops: vec![],
                        applied: false,
                    };
                }
                t
            };
            let moved_rows = rows[*start..=*end].to_vec();
            MutationResult {
                ops: vec![
                    TableOp::BulkRowDelete {
                        start: *start,
                        end: *end,
                        data: moved_rows.clone(),
                    },
                    TableOp::BulkRowAdd {
                        start: target_start,
                        data: moved_rows,
                    },
                ],
                applied: true,
            }
        }
        CsvMutationRequest::MoveColumns {
            start,
            end,
            direction,
        } => {
            let count = end.saturating_sub(*start) + 1;
            let target_start = if *direction < 0 {
                if *start == 0 {
                    return MutationResult {
                        ops: vec![],
                        applied: false,
                    };
                }
                *start - 1
            } else {
                let t = *start + 1;
                if t + count > headers.len() {
                    return MutationResult {
                        ops: vec![],
                        applied: false,
                    };
                }
                t
            };
            let moved_headers: Vec<String> = headers[*start..=*end].to_vec();
            if moved_headers.is_empty() {
                return MutationResult {
                    ops: vec![],
                    applied: false,
                };
            }
            let moved_data: Vec<Vec<String>> = rows
                .iter()
                .map(|row| {
                    let end_idx = (*end + 1).min(row.len());
                    if *start < row.len() {
                        row[*start..end_idx].to_vec()
                    } else {
                        Vec::new()
                    }
                })
                .collect();
            MutationResult {
                ops: vec![
                    TableOp::BulkColDelete {
                        start: *start,
                        end: *end,
                        headers: moved_headers.clone(),
                        data: moved_data.clone(),
                    },
                    TableOp::BulkColAdd {
                        start: target_start,
                        headers: moved_headers,
                        data: moved_data,
                    },
                ],
                applied: true,
            }
        }
    }
}
