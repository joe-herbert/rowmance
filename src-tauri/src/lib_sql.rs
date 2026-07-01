/// SQL text utilities shared across command handlers.
/// Mirrors the logic in the frontend's `src/lib/utils/sql.ts`.
///
/// Split a SQL document into individual statements by semicolons,
/// respecting single-quoted strings, double-quoted identifiers,
/// backtick-quoted identifiers, and -- / block comments.
pub fn split_sql_statements(sql: &str) -> Vec<String> {
    let mut statements: Vec<String> = Vec::new();
    let mut current = String::new();
    let chars: Vec<char> = sql.chars().collect();
    let len = chars.len();
    let mut i = 0;

    while i < len {
        let ch = chars[i];
        let next = chars.get(i + 1).copied();

        // Block comment: /* ... */
        if ch == '/' && next == Some('*') {
            let start = i;
            i += 2;
            while i < len {
                if chars[i] == '*' && chars.get(i + 1).copied() == Some('/') {
                    i += 2;
                    break;
                }
                i += 1;
            }
            current.extend(chars[start..i].iter());
            continue;
        }

        // Single-line comment: -- ...
        if ch == '-' && next == Some('-') {
            let start = i;
            while i < len && chars[i] != '\n' {
                i += 1;
            }
            if i < len {
                i += 1; // consume newline
            }
            current.extend(chars[start..i].iter());
            continue;
        }

        // Single-quoted string
        if ch == '\'' {
            let start = i;
            i += 1;
            while i < len {
                if chars[i] == '\'' && chars.get(i + 1).copied() == Some('\'') {
                    i += 2; // escaped quote
                } else if chars[i] == '\'' {
                    i += 1;
                    break;
                } else {
                    i += 1;
                }
            }
            current.extend(chars[start..i].iter());
            continue;
        }

        // Double-quoted identifier
        if ch == '"' {
            let start = i;
            i += 1;
            while i < len && chars[i] != '"' {
                i += 1;
            }
            if i < len {
                i += 1;
            }
            current.extend(chars[start..i].iter());
            continue;
        }

        // Backtick-quoted identifier
        if ch == '`' {
            let start = i;
            i += 1;
            while i < len && chars[i] != '`' {
                i += 1;
            }
            if i < len {
                i += 1;
            }
            current.extend(chars[start..i].iter());
            continue;
        }

        // Statement delimiter
        if ch == ';' {
            let trimmed = current.trim().to_owned();
            if !trimmed.is_empty() {
                statements.push(trimmed);
            }
            current.clear();
            i += 1;
            continue;
        }

        current.push(ch);
        i += 1;
    }

    let trailing = current.trim().to_owned();
    if !trailing.is_empty() {
        statements.push(trailing);
    }

    statements
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_simple_statements() {
        let sql = "SELECT 1; SELECT 2;";
        let stmts = split_sql_statements(sql);
        assert_eq!(stmts, vec!["SELECT 1", "SELECT 2"]);
    }

    #[test]
    fn ignores_semicolons_in_strings() {
        let sql = "SELECT 'a;b'; SELECT 2;";
        let stmts = split_sql_statements(sql);
        assert_eq!(stmts, vec!["SELECT 'a;b'", "SELECT 2"]);
    }

    #[test]
    fn ignores_semicolons_in_block_comments() {
        let sql = "/* comment; here */ SELECT 1;";
        let stmts = split_sql_statements(sql);
        assert_eq!(stmts.len(), 1);
        assert!(stmts[0].contains("SELECT 1"));
    }

    #[test]
    fn trailing_statement_without_semicolon() {
        let sql = "SELECT 1";
        let stmts = split_sql_statements(sql);
        assert_eq!(stmts, vec!["SELECT 1"]);
    }

    #[test]
    fn empty_input() {
        assert!(split_sql_statements("").is_empty());
        assert!(split_sql_statements("  ").is_empty());
    }

    // ── Additional tests requested in audit ──────────────────────────────────

    #[test]
    fn single_statement_no_semicolon_returns_one() {
        let stmts = split_sql_statements("SELECT id FROM users");
        assert_eq!(stmts.len(), 1);
        assert_eq!(stmts[0], "SELECT id FROM users");
    }

    #[test]
    fn two_semicolon_separated_statements_returns_two() {
        let stmts = split_sql_statements("SELECT 1; SELECT 2");
        assert_eq!(stmts.len(), 2);
        assert_eq!(stmts[0], "SELECT 1");
        assert_eq!(stmts[1], "SELECT 2");
    }

    #[test]
    fn semicolon_inside_string_literal_not_a_split_point() {
        let stmts = split_sql_statements("SELECT 'hello;world'");
        assert_eq!(stmts.len(), 1);
        assert_eq!(stmts[0], "SELECT 'hello;world'");
    }

    #[test]
    fn semicolon_inside_block_comment_not_a_split_point() {
        let stmts = split_sql_statements("/* a;b;c */ SELECT 42");
        assert_eq!(stmts.len(), 1);
        assert!(stmts[0].contains("SELECT 42"));
    }

    #[test]
    fn empty_input_returns_empty_vec() {
        assert!(split_sql_statements("").is_empty());
    }

    #[test]
    fn whitespace_only_statements_are_filtered_out() {
        // Two real statements separated by whitespace-only content.
        let stmts = split_sql_statements("SELECT 1;   ;SELECT 2;");
        // The whitespace-only "statement" in the middle should be dropped.
        assert_eq!(stmts.len(), 2);
        assert_eq!(stmts[0], "SELECT 1");
        assert_eq!(stmts[1], "SELECT 2");
    }

    #[test]
    fn semicolon_inside_line_comment_not_a_split_point() {
        let stmts = split_sql_statements("SELECT 1 -- ignore; this\n; SELECT 2");
        assert_eq!(stmts.len(), 2);
        assert!(stmts[0].contains("SELECT 1"));
        assert_eq!(stmts[1], "SELECT 2");
    }

    #[test]
    fn semicolons_inside_backtick_identifiers_not_split_points() {
        let stmts = split_sql_statements("SELECT `col;name` FROM t; SELECT 1");
        assert_eq!(stmts.len(), 2);
        assert!(stmts[0].contains("`col;name`"));
    }
}
