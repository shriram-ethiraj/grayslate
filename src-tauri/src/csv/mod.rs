//! CSV table mode backend.
//!
//! This module provides the Rust-backed session engine for CSV table editing.
//! Parsing, serialization, structural mutations, and undo/redo all happen here,
//! with the frontend communicating via Tauri IPC commands defined in
//! `commands/csv.rs`.
//!
//! ## Module layout
//!
//! - [`types`] — Response/snapshot types serialized to the frontend.
//! - [`ops`] — `TableOp` enum, inversion, and model application.
//! - [`mutation`] — `CsvMutationRequest` deserialization and op building.
//! - [`session`] — `CsvSession` with history, commit, undo/redo, flush.
//! - [`parser`] — CSV parsing, delimiter detection, serialization.

pub mod mutation;
pub mod ops;
pub mod parser;
pub mod session;
pub mod types;

// Re-export the public API used by commands/csv.rs.
pub use mutation::CsvMutationRequest;
pub use parser::parse_csv;
pub use session::CsvSession;
pub use types::{CsvMutationResponse, CsvRowWindow, CsvTableSnapshot};

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::mutation::CsvMutationRequest;
    use super::parser::{detect_delimiter, parse_csv, serialize_csv};
    use super::session::CsvSession;
    use std::sync::atomic::AtomicBool;

    fn no_cancel() -> AtomicBool {
        AtomicBool::new(false)
    }

    fn parse_simple(text: &str) -> CsvSession {
        parse_csv(text, &no_cancel(), |_| {}).unwrap()
    }

    // -- Parsing -----------------------------------------------------------

    #[test]
    fn parse_basic_csv() {
        let session = parse_simple("name,age\nAlice,30\nBob,25");
        assert_eq!(session.headers, vec!["name", "age"]);
        assert_eq!(session.rows.len(), 2);
        assert_eq!(session.rows[0], vec!["Alice", "30"]);
        assert_eq!(session.rows[1], vec!["Bob", "25"]);
        assert_eq!(session.delimiter, b',');
        assert!(session.live_mirror_enabled);
    }

    #[test]
    fn parse_tab_delimited() {
        let session = parse_simple("a\tb\tc\n1\t2\t3");
        assert_eq!(session.delimiter, b'\t');
        assert_eq!(session.headers, vec!["a", "b", "c"]);
        assert_eq!(session.rows[0], vec!["1", "2", "3"]);
    }

    #[test]
    fn parse_empty_text() {
        let session = parse_simple("");
        assert!(session.headers.is_empty());
        assert!(session.rows.is_empty());
        assert!(session.live_mirror_enabled);
    }

    #[test]
    fn parse_with_quoted_fields() {
        let session = parse_simple("h1,h2\n\"hello, world\",bar");
        assert_eq!(session.rows[0][0], "hello, world");
        assert_eq!(session.rows[0][1], "bar");
    }

    #[test]
    fn parse_skips_empty_lines() {
        let session = parse_simple("a,b\n1,2\n\n3,4\n  \n5,6");
        assert_eq!(session.rows.len(), 3);
    }

    // -- Delimiter detection -----------------------------------------------

    #[test]
    fn detect_comma() {
        assert_eq!(detect_delimiter("a,b,c"), b',');
    }

    #[test]
    fn detect_tab() {
        assert_eq!(detect_delimiter("a\tb\tc"), b'\t');
    }

    #[test]
    fn detect_semicolon() {
        assert_eq!(detect_delimiter("a;b;c"), b';');
    }

    #[test]
    fn detect_ignores_quoted() {
        assert_eq!(detect_delimiter("\"a,b,c\";d;e;f"), b';');
    }

    #[test]
    fn detect_defaults_comma() {
        assert_eq!(detect_delimiter("abc"), b',');
    }

    // -- Serialization roundtrip -------------------------------------------

    #[test]
    fn serialize_roundtrip() {
        let text = "name,age\nAlice,30\nBob,25";
        let session = parse_simple(text);
        let output = serialize_csv(&session.headers, &session.rows, session.delimiter);
        assert_eq!(output, text);
    }

    #[test]
    fn serialize_with_quoting() {
        let headers = vec!["h1".to_string(), "h2".to_string()];
        let rows = vec![vec!["hello, world".to_string(), "bar".to_string()]];
        let output = serialize_csv(&headers, &rows, b',');
        assert_eq!(output, "h1,h2\n\"hello, world\",bar");
    }

    // -- Mutations ---------------------------------------------------------

    #[test]
    fn cell_edit() {
        let mut session = parse_simple("a,b\n1,2\n3,4");
        let resp = session.mutate(
            &CsvMutationRequest::EditCell {
                row_index: 0,
                col_index: 1,
                new_value: "X".to_string(),
            },
            "edit.table",
        );
        assert!(resp.applied);
        assert_eq!(session.rows[0][1], "X");
        assert_eq!(session.version, 2);
    }

    #[test]
    fn cell_edit_noop() {
        let mut session = parse_simple("a,b\n1,2");
        let resp = session.mutate(
            &CsvMutationRequest::EditCell {
                row_index: 0,
                col_index: 0,
                new_value: "1".to_string(),
            },
            "edit.table",
        );
        assert!(!resp.applied);
        assert_eq!(session.version, 1);
    }

    #[test]
    fn header_edit() {
        let mut session = parse_simple("a,b\n1,2");
        let resp = session.mutate(
            &CsvMutationRequest::EditHeader {
                col_index: 0,
                new_value: "A".to_string(),
            },
            "edit.table",
        );
        assert!(resp.applied);
        assert_eq!(session.headers[0], "A");
    }

    #[test]
    fn add_and_delete_row() {
        let mut session = parse_simple("a,b\n1,2\n3,4");
        assert_eq!(session.rows.len(), 2);

        session.mutate(
            &CsvMutationRequest::AddRow { index: 1 },
            "add.table",
        );
        assert_eq!(session.rows.len(), 3);
        assert_eq!(session.rows[1], vec!["", ""]);

        session.mutate(
            &CsvMutationRequest::DeleteRows { start: 1, end: 1 },
            "delete.table",
        );
        assert_eq!(session.rows.len(), 2);
        assert_eq!(session.rows[0], vec!["1", "2"]);
        assert_eq!(session.rows[1], vec!["3", "4"]);
    }

    #[test]
    fn add_and_delete_column() {
        let mut session = parse_simple("a,b\n1,2\n3,4");
        assert_eq!(session.headers.len(), 2);

        session.mutate(
            &CsvMutationRequest::AddColumn { index: 1 },
            "add.table",
        );
        assert_eq!(session.headers.len(), 3);
        assert_eq!(session.headers, vec!["a", "", "b"]);
        assert_eq!(session.rows[0], vec!["1", "", "2"]);

        session.mutate(
            &CsvMutationRequest::DeleteColumns { start: 1, end: 1 },
            "delete.table",
        );
        assert_eq!(session.headers.len(), 2);
        assert_eq!(session.headers, vec!["a", "b"]);
        assert_eq!(session.rows[0], vec!["1", "2"]);
    }

    #[test]
    fn clear_selection() {
        let mut session = parse_simple("a,b,c\n1,2,3\n4,5,6\n7,8,9");
        session.mutate(
            &CsvMutationRequest::ClearSelection {
                start_row: 0,
                end_row: 1,
                start_col: 1,
                end_col: 2,
            },
            "clear.table",
        );
        assert_eq!(session.rows[0], vec!["1", "", ""]);
        assert_eq!(session.rows[1], vec!["4", "", ""]);
        assert_eq!(session.rows[2], vec!["7", "8", "9"]);
    }

    #[test]
    fn move_rows() {
        let mut session = parse_simple("h\na\nb\nc");
        session.mutate(
            &CsvMutationRequest::MoveRows {
                start: 0,
                end: 0,
                direction: 1,
            },
            "move.table",
        );
        assert_eq!(session.rows[0], vec!["b"]);
        assert_eq!(session.rows[1], vec!["a"]);
    }

    #[test]
    fn move_columns() {
        let mut session = parse_simple("a,b,c\n1,2,3");
        session.mutate(
            &CsvMutationRequest::MoveColumns {
                start: 0,
                end: 0,
                direction: 1,
            },
            "move.table",
        );
        assert_eq!(session.headers, vec!["b", "a", "c"]);
        assert_eq!(session.rows[0], vec!["2", "1", "3"]);
    }

    // -- Undo / redo -------------------------------------------------------

    #[test]
    fn undo_redo_cell_edit() {
        let mut session = parse_simple("a,b\n1,2");
        session.mutate(
            &CsvMutationRequest::EditCell {
                row_index: 0,
                col_index: 0,
                new_value: "X".to_string(),
            },
            "edit.table",
        );
        assert_eq!(session.rows[0][0], "X");

        let resp = session.undo();
        assert!(resp.applied);
        assert_eq!(session.rows[0][0], "1");

        let resp = session.redo();
        assert!(resp.applied);
        assert_eq!(session.rows[0][0], "X");
    }

    #[test]
    fn undo_column_add() {
        let mut session = parse_simple("a,b\n1,2\n3,4");
        session.mutate(
            &CsvMutationRequest::AddColumn { index: 2 },
            "add.table",
        );
        assert_eq!(session.headers.len(), 3);

        session.undo();
        assert_eq!(session.headers.len(), 2);
        assert_eq!(session.headers, vec!["a", "b"]);
        assert_eq!(session.rows[0], vec!["1", "2"]);
    }

    #[test]
    fn undo_column_delete() {
        let mut session = parse_simple("a,b,c\n1,2,3\n4,5,6");
        session.mutate(
            &CsvMutationRequest::DeleteColumns { start: 1, end: 1 },
            "delete.table",
        );
        assert_eq!(session.headers, vec!["a", "c"]);

        session.undo();
        assert_eq!(session.headers, vec!["a", "b", "c"]);
        assert_eq!(session.rows[0], vec!["1", "2", "3"]);
    }

    #[test]
    fn undo_empty_stack() {
        let mut session = parse_simple("a\n1");
        let resp = session.undo();
        assert!(!resp.applied);
    }

    #[test]
    fn redo_empty_stack() {
        let mut session = parse_simple("a\n1");
        let resp = session.redo();
        assert!(!resp.applied);
    }

    #[test]
    fn undo_redo_move_rows() {
        let mut session = parse_simple("h\na\nb\nc");
        session.mutate(
            &CsvMutationRequest::MoveRows {
                start: 0,
                end: 0,
                direction: 1,
            },
            "move.table",
        );
        assert_eq!(session.rows[0], vec!["b"]);
        assert_eq!(session.rows[1], vec!["a"]);

        session.undo();
        assert_eq!(session.rows[0], vec!["a"]);
        assert_eq!(session.rows[1], vec!["b"]);

        session.redo();
        assert_eq!(session.rows[0], vec!["b"]);
        assert_eq!(session.rows[1], vec!["a"]);
    }

    #[test]
    fn history_cap() {
        let mut session = parse_simple("a\n1");
        for i in 0..250 {
            session.mutate(
                &CsvMutationRequest::EditCell {
                    row_index: 0,
                    col_index: 0,
                    new_value: format!("{}", i),
                },
                "edit.table",
            );
        }
        // History should be capped at 200.
        let mut undo_count = 0;
        loop {
            let resp = session.undo();
            if !resp.applied {
                break;
            }
            undo_count += 1;
        }
        assert_eq!(undo_count, 200);
    }

    // -- Live mirror -------------------------------------------------------

    #[test]
    fn live_mirror_returns_text() {
        let mut session = parse_simple("a,b\n1,2");
        assert!(session.live_mirror_enabled);
        let resp = session.mutate(
            &CsvMutationRequest::EditCell {
                row_index: 0,
                col_index: 0,
                new_value: "X".to_string(),
            },
            "edit.table",
        );
        assert!(resp.mirror_text.is_some());
        let text = resp.mirror_text.unwrap();
        assert!(text.contains("X"));
    }

    #[test]
    fn non_mirror_clears_text() {
        let text_100k = {
            let mut s = String::from("a\n");
            for i in 0..100_001 {
                s.push_str(&format!("{}\n", i));
            }
            s
        };
        let mut session = parse_csv(&text_100k, &no_cancel(), |_| {}).unwrap();
        assert!(!session.live_mirror_enabled);

        let resp = session.mutate(
            &CsvMutationRequest::EditCell {
                row_index: 0,
                col_index: 0,
                new_value: "X".to_string(),
            },
            "edit.table",
        );
        assert!(resp.mirror_text.is_none());
        assert!(session.serialized_text_is_empty());
    }

    // -- Flush -------------------------------------------------------------

    #[test]
    fn flush_regenerates_text() {
        let mut session = parse_simple("a,b\n1,2");
        session.mutate(
            &CsvMutationRequest::EditCell {
                row_index: 0,
                col_index: 0,
                new_value: "X".to_string(),
            },
            "edit.table",
        );
        let flushed = session.flush_text();
        assert_eq!(flushed, "a,b\nX,2");
    }

    #[test]
    fn flush_idempotent() {
        let mut session = parse_simple("a,b\n1,2");
        let t1 = session.flush_text();
        let t2 = session.flush_text();
        assert_eq!(t1, t2);
    }

    // -- Cancellation ------------------------------------------------------

    #[test]
    fn parse_cancelled() {
        let cancelled = AtomicBool::new(true);
        let result = parse_csv("a,b\n1,2", &cancelled, |_| {});
        match result {
            Ok(_) => panic!("Expected cancellation error"),
            Err(e) => assert!(e.contains("cancelled"), "unexpected error: {}", e),
        }
    }
}
