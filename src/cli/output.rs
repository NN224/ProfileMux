pub struct Table {
    headers: Vec<String>,
    rows: Vec<Vec<String>>,
}

impl Table {
    pub fn new(headers: Vec<String>) -> Self {
        Table {
            headers,
            rows: Vec::new(),
        }
    }

    pub fn add_row(&mut self, row: Vec<String>) {
        self.rows.push(row);
    }

    pub fn render(&self) -> String {
        if self.headers.is_empty() {
            return String::new();
        }
        let widths = column_widths(&self.headers, &self.rows);
        let mut out = String::new();
        out.push_str(&render_line(&self.headers, &widths));
        out.push('\n');
        for row in &self.rows {
            out.push_str(&render_line(row, &widths));
            out.push('\n');
        }
        out
    }

    pub fn print(&self) {
        print!("{}", self.render());
    }
}

fn column_widths(headers: &[String], rows: &[Vec<String>]) -> Vec<usize> {
    headers
        .iter()
        .enumerate()
        .map(|(i, h)| {
            let max_row_len = rows
                .iter()
                .filter_map(|r| r.get(i))
                .map(|cell| cell.chars().count())
                .max()
                .unwrap_or(0);
            h.chars().count().max(max_row_len)
        })
        .collect()
}

fn render_line(cells: &[String], widths: &[usize]) -> String {
    let mut line = String::new();
    for (i, width) in widths.iter().enumerate() {
        let cell = cells.get(i).map(String::as_str).unwrap_or("");
        if i > 0 {
            line.push_str("  ");
        }
        if i + 1 == widths.len() {
            line.push_str(cell);
        } else {
            let cell_char_count = cell.chars().count();
            line.push_str(cell);
            if *width > cell_char_count {
                let pad = *width - cell_char_count;
                line.push_str(&" ".repeat(pad));
            }
        }
    }
    line
}

pub fn print_json<T: serde::Serialize>(value: &T) -> anyhow::Result<()> {
    let json = serde_json::to_string_pretty(value)?;
    println!("{json}");
    Ok(())
}
