//! Bounded UTF-8 CSV reader: quoted delimiters, escaped quotes and multiline fields; no formula evaluation or silent row loss.
use veyndo_core::{Result, bad};
/// The header and one to five hundred source rows, with the separator the header line uses most.
pub fn rows(source: &str) -> Result<Vec<Vec<String>>> {
    let source = source.trim_start_matches('\u{feff}');
    let line = source.lines().next().unwrap_or_default();
    let sep = if line.matches(';').count() > line.matches(',').count() {
        ';'
    } else {
        ','
    };
    let mut cells = vec![];
    let mut rows = vec![];
    let mut cell = String::new();
    let mut quoted = false;
    let mut closed = false;
    let mut chars = source.chars().peekable();
    while let Some(ch) = chars.next() {
        if quoted {
            if ch == '"' {
                if chars.peek() == Some(&'"') {
                    chars.next();
                    cell.push('"');
                } else {
                    quoted = false;
                    closed = true;
                }
            } else {
                cell.push(ch);
            }
        } else if ch == '"' && cell.is_empty() && !closed {
            quoted = true;
        } else if ch == sep || ch == '\n' || ch == '\r' {
            cells.push(std::mem::take(&mut cell));
            closed = false;
            if ch != sep {
                if ch == '\r' && chars.peek() == Some(&'\n') {
                    chars.next();
                }
                if cells.iter().any(|c| !c.trim().is_empty()) {
                    rows.push(std::mem::take(&mut cells));
                } else {
                    cells.clear();
                }
                if rows.len() > 501 {
                    return Err(bad(
                        "CSV supports at most 500 source rows per reviewed batch",
                    ));
                }
            }
        } else if closed {
            return Err(bad("CSV has text after a closing quote"));
        } else {
            cell.push(ch);
        }
    }
    if quoted {
        return Err(bad("CSV has an unclosed quoted field"));
    }
    cells.push(cell);
    if cells.iter().any(|c| !c.trim().is_empty()) {
        rows.push(cells);
    }
    if rows.len() < 2 || rows.len() > 501 {
        return Err(bad("CSV needs a header and 1..500 source rows"));
    }
    Ok(rows)
}
#[cfg(test)]
mod tests {
    use super::rows;
    #[test]
    fn reads_quoted_cells_and_refuses_broken_or_unbounded_sources() {
        let r = rows("\u{feff}name;price\r\n\"Oak; chair\";\"19,90\"\r\n").unwrap();
        assert_eq!(r[1], vec!["Oak; chair".to_string(), "19,90".to_string()]);
        assert!(rows("name,price").is_err());
        assert!(rows("name,price\n\"open,3").is_err());
        assert!(rows(&format!("name,price\n{}", "chair,1\n".repeat(501))).is_err());
    }
}
