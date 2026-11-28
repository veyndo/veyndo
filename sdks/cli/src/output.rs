//! Renders results as aligned tables for people or as JSON (`--json`) for scripts; status lines go to stderr.
use serde_json::Value;

/// Output mode of one invocation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    /// Human-readable tables and sentences.
    Table,
    /// The server's JSON (or the command's JSON summary) on stdout.
    Json,
}

/// One table column: a header and how to read the cell from a row.
pub struct Column {
    /// Header text.
    pub header: &'static str,
    /// JSON pointers tried in order; the first non-null value is shown.
    pub pointers: &'static [&'static str],
}

/// A column reading the first present pointer.
pub const fn column(header: &'static str, pointers: &'static [&'static str]) -> Column {
    Column { header, pointers }
}

/// Plain text of a JSON cell: strings unquoted, objects compact, absent values as `-`.
pub fn cell(value: Option<&Value>) -> String {
    match value {
        None | Some(Value::Null) => "-".into(),
        Some(Value::String(text)) => text.replace(['\n', '\t'], " "),
        Some(Value::Array(items)) => items
            .iter()
            .map(|item| cell(Some(item)))
            .collect::<Vec<_>>()
            .join(","),
        Some(other) => other.to_string(),
    }
}

fn read<'a>(row: &'a Value, pointers: &[&str]) -> Option<&'a Value> {
    pointers
        .iter()
        .filter_map(|pointer| row.pointer(pointer))
        .find(|value| !value.is_null())
}

/// Formats rows as an aligned text table; long cells are shortened to keep one row per line.
pub fn table(rows: &[Value], columns: &[Column]) -> String {
    const WIDEST: usize = 48;
    let cells: Vec<Vec<String>> = rows
        .iter()
        .map(|row| {
            columns
                .iter()
                .map(|c| shorten(&cell(read(row, c.pointers)), WIDEST))
                .collect()
        })
        .collect();
    let widths: Vec<usize> = columns
        .iter()
        .enumerate()
        .map(|(i, c)| {
            cells
                .iter()
                .map(|row| row[i].chars().count())
                .chain([c.header.len()])
                .max()
                .unwrap_or(0)
        })
        .collect();
    let line = |values: Vec<&str>| {
        values
            .iter()
            .zip(&widths)
            .map(|(value, width)| format!("{value:<width$}"))
            .collect::<Vec<_>>()
            .join("  ")
            .trim_end()
            .to_string()
    };
    let mut out = vec![line(columns.iter().map(|c| c.header).collect())];
    out.extend(
        cells
            .iter()
            .map(|row| line(row.iter().map(String::as_str).collect())),
    );
    out.join("\n")
}

fn shorten(text: &str, limit: usize) -> String {
    if text.chars().count() <= limit {
        return text.to_string();
    }
    let mut short: String = text.chars().take(limit - 1).collect();
    short.push('…');
    short
}

/// Prints rows as a table, or the JSON value in JSON mode.
pub fn rows(format: Format, value: &Value, rows: &[Value], columns: &[Column]) {
    match format {
        Format::Json => json(value),
        Format::Table if rows.is_empty() => eprintln!("No results."),
        Format::Table => println!("{}", table(rows, columns)),
    }
}

/// Prints a JSON value on stdout.
pub fn json(value: &Value) {
    println!(
        "{}",
        serde_json::to_string_pretty(value).unwrap_or_default()
    );
}

/// Prints `key: value` lines, or the JSON value in JSON mode.
pub fn fields(format: Format, value: &Value, lines: &[(&str, String)]) {
    match format {
        Format::Json => json(value),
        Format::Table => {
            let width = lines.iter().map(|(k, _)| k.len()).max().unwrap_or(0);
            for (key, text) in lines {
                println!("{key:<width$}  {text}");
            }
        }
    }
}

/// A status line for people; stderr, so `--json` stdout stays parseable.
pub fn note(text: &str) {
    eprintln!("{text}");
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn tables_align_and_fall_back_to_alternative_pointers() {
        let rows = [
            json!({"id":"a","name":{"en-GB":"Lamp"},"price":12.5}),
            json!({"id":"bbbb","title":"Desk","price":null}),
        ];
        let text = table(
            &rows,
            &[
                column("ID", &["/id"]),
                column("NAME", &["/name/en-GB", "/title"]),
                column("PRICE", &["/price"]),
            ],
        );
        assert_eq!(text, "ID    NAME  PRICE\na     Lamp  12.5\nbbbb  Desk  -");
    }

    #[test]
    fn long_cells_are_shortened() {
        assert_eq!(shorten("abcdef", 4), "abc…");
        assert_eq!(cell(Some(&json!(["a", 1]))), "a,1");
    }
}
