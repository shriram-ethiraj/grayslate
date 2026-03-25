use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// TableOp — structural mutations stored for undo/redo
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum TableOp {
    Cell {
        row: usize,
        col: usize,
        old_value: String,
        new_value: String,
    },
    HeaderCell {
        col: usize,
        old_value: String,
        new_value: String,
    },
    RowAdd {
        index: usize,
        data: Vec<String>,
    },
    RowDelete {
        index: usize,
        data: Vec<String>,
    },
    BulkRowDelete {
        start: usize,
        end: usize,
        data: Vec<Vec<String>>,
    },
    BulkRowAdd {
        start: usize,
        data: Vec<Vec<String>>,
    },
    BulkColDelete {
        start: usize,
        end: usize,
        headers: Vec<String>,
        data: Vec<Vec<String>>,
    },
    BulkColAdd {
        start: usize,
        headers: Vec<String>,
        data: Vec<Vec<String>>,
    },
    BulkCellClear {
        start_row: usize,
        end_row: usize,
        start_col: usize,
        end_col: usize,
        old_values: Vec<Vec<String>>,
    },
    BulkCellFill {
        start_row: usize,
        end_row: usize,
        start_col: usize,
        end_col: usize,
        data: Vec<Vec<String>>,
    },
}

fn clone_rows(rows: &[Vec<String>]) -> Vec<Vec<String>> {
    rows.iter().map(|r| r.clone()).collect()
}

pub fn invert_op(op: &TableOp) -> TableOp {
    match op {
        TableOp::Cell {
            row,
            col,
            old_value,
            new_value,
        } => TableOp::Cell {
            row: *row,
            col: *col,
            old_value: new_value.clone(),
            new_value: old_value.clone(),
        },
        TableOp::HeaderCell {
            col,
            old_value,
            new_value,
        } => TableOp::HeaderCell {
            col: *col,
            old_value: new_value.clone(),
            new_value: old_value.clone(),
        },
        TableOp::RowAdd { index, data } => TableOp::RowDelete {
            index: *index,
            data: data.clone(),
        },
        TableOp::RowDelete { index, data } => TableOp::RowAdd {
            index: *index,
            data: data.clone(),
        },
        TableOp::BulkRowDelete { start, data, .. } => TableOp::BulkRowAdd {
            start: *start,
            data: clone_rows(data),
        },
        TableOp::BulkRowAdd { start, data } => TableOp::BulkRowDelete {
            start: *start,
            end: *start + data.len().saturating_sub(1),
            data: clone_rows(data),
        },
        TableOp::BulkColDelete {
            start,
            headers,
            data,
            ..
        } => TableOp::BulkColAdd {
            start: *start,
            headers: headers.clone(),
            data: clone_rows(data),
        },
        TableOp::BulkColAdd {
            start,
            headers,
            data,
        } => TableOp::BulkColDelete {
            start: *start,
            end: *start + headers.len().saturating_sub(1),
            headers: headers.clone(),
            data: clone_rows(data),
        },
        TableOp::BulkCellClear {
            start_row,
            end_row,
            start_col,
            end_col,
            old_values,
        } => TableOp::BulkCellFill {
            start_row: *start_row,
            end_row: *end_row,
            start_col: *start_col,
            end_col: *end_col,
            data: clone_rows(old_values),
        },
        TableOp::BulkCellFill {
            start_row,
            end_row,
            start_col,
            end_col,
            data,
        } => TableOp::BulkCellClear {
            start_row: *start_row,
            end_row: *end_row,
            start_col: *start_col,
            end_col: *end_col,
            old_values: clone_rows(data),
        },
    }
}

pub fn invert_ops(ops: &[TableOp]) -> Vec<TableOp> {
    ops.iter().rev().map(invert_op).collect()
}

// ---------------------------------------------------------------------------
// Table model mutation
// ---------------------------------------------------------------------------

pub struct TableModel<'a> {
    pub headers: &'a mut Vec<String>,
    pub rows: &'a mut Vec<Vec<String>>,
}

fn ensure_row_width(row: &mut Vec<String>, col: usize) {
    while row.len() <= col {
        row.push(String::new());
    }
}

pub fn apply_ops(model: &mut TableModel, ops: &[TableOp]) {
    for op in ops {
        match op {
            TableOp::Cell {
                row,
                col,
                new_value,
                ..
            } => {
                if let Some(r) = model.rows.get_mut(*row) {
                    ensure_row_width(r, *col);
                    r[*col] = new_value.clone();
                }
            }
            TableOp::HeaderCell { col, new_value, .. } => {
                while model.headers.len() <= *col {
                    model.headers.push(String::new());
                }
                model.headers[*col] = new_value.clone();
            }
            TableOp::RowAdd { index, data } => {
                let idx = (*index).min(model.rows.len());
                model.rows.insert(idx, data.clone());
            }
            TableOp::RowDelete { index, .. } => {
                if *index < model.rows.len() {
                    model.rows.remove(*index);
                }
            }
            TableOp::BulkRowDelete { start, end, .. } => {
                let s = (*start).min(model.rows.len());
                let e = (*end + 1).min(model.rows.len());
                if s < e {
                    model.rows.drain(s..e);
                }
            }
            TableOp::BulkRowAdd { start, data } => {
                let idx = (*start).min(model.rows.len());
                // Reserve space and splice in one pass.
                let new_rows: Vec<Vec<String>> = clone_rows(data);
                let tail = model.rows.split_off(idx);
                model.rows.reserve(new_rows.len() + tail.len());
                model.rows.extend(new_rows);
                model.rows.extend(tail);
            }
            TableOp::BulkColDelete {
                start, end, ..
            } => {
                let s = *start;
                let e = *end;
                let count = e.saturating_sub(s) + 1;
                if s < model.headers.len() {
                    let drain_end = (s + count).min(model.headers.len());
                    model.headers.drain(s..drain_end);
                }
                for row in model.rows.iter_mut() {
                    if s < row.len() {
                        let drain_end = (s + count).min(row.len());
                        row.drain(s..drain_end);
                    }
                }
                if model.headers.is_empty() {
                    model.rows.clear();
                }
            }
            TableOp::BulkColAdd {
                start,
                headers,
                data,
            } => {
                let idx = (*start).min(model.headers.len());
                // Splice headers.
                let header_tail = model.headers.split_off(idx);
                model.headers.reserve(headers.len() + header_tail.len());
                model.headers.extend_from_slice(headers);
                model.headers.extend(header_tail);

                // Ensure enough rows exist.
                while model.rows.len() < data.len() {
                    model.rows.push(Vec::new());
                }

                for (i, row) in model.rows.iter_mut().enumerate() {
                    let row_data = data.get(i).cloned().unwrap_or_default();
                    let col_idx = idx.min(row.len());
                    let tail = row.split_off(col_idx);
                    row.reserve(row_data.len() + tail.len());
                    row.extend(row_data);
                    row.extend(tail);
                }
            }
            TableOp::BulkCellClear {
                start_row,
                end_row,
                start_col,
                end_col,
                ..
            } => {
                for ri in *start_row..=*end_row {
                    if let Some(row) = model.rows.get_mut(ri) {
                        for ci in *start_col..=*end_col {
                            ensure_row_width(row, ci);
                            row[ci] = String::new();
                        }
                    }
                }
            }
            TableOp::BulkCellFill {
                start_row,
                end_row,
                start_col,
                end_col,
                data,
            } => {
                for ri in *start_row..=*end_row {
                    if let Some(row) = model.rows.get_mut(ri) {
                        let data_row = data.get(ri - start_row);
                        for ci in *start_col..=*end_col {
                            ensure_row_width(row, ci);
                            row[ci] = data_row
                                .and_then(|dr| dr.get(ci - start_col))
                                .cloned()
                                .unwrap_or_default();
                        }
                    }
                }
            }
        }
    }
}
