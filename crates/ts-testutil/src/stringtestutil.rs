use ts_stringutil::{guess_indentation, is_white_space_like};

pub fn dedent(text: &str) -> String {
    let mut lines: Vec<String> = text.split('\n').map(str::to_owned).collect();
    // Remove blank lines in the beginning and end
    // and convert all tabs in the beginning of line to spaces
    let mut start_line: isize = -1;
    let mut last_line = 0;
    for (i, line) in lines.iter_mut().enumerate() {
        let first_non_white = line.find(|r| !is_white_space_like(r));
        if let Some(first_non_white) = first_non_white
            && first_non_white > 0
        {
            *line = line[0..first_non_white].replace('\t', "    ") + &line[first_non_white..];
        }
        let trimmed = line.trim();
        if !trimmed.is_empty() {
            if start_line == -1 {
                start_line = i as isize;
            }
            last_line = i;
        }
    }
    if start_line == -1 {
        panic!("slice bounds out of range");
    }
    let start_line = start_line as usize;
    let mut lines = lines[start_line..last_line + 1].to_vec();
    let mapped_lines: Vec<String> = lines
        .iter()
        .map(|line| {
            if line.trim().is_empty() {
                String::new()
            } else {
                line.clone()
            }
        })
        .collect();
    let mapped_line_refs: Vec<&str> = mapped_lines.iter().map(String::as_str).collect();
    let indentation = guess_indentation(&mapped_line_refs);
    if indentation > 0 {
        for line in &mut lines {
            if line.len() > indentation {
                *line = line[indentation..].to_owned();
            } else {
                line.clear();
            }
        }
    }
    lines.join("\n")
}
