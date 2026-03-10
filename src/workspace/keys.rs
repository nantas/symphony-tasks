pub fn sanitize_issue_key(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let mut last_was_dash = false;

    for ch in input.chars() {
        let mapped = if ch.is_ascii_alphanumeric() {
            last_was_dash = false;
            Some(ch.to_ascii_lowercase())
        } else if !last_was_dash {
            last_was_dash = true;
            Some('-')
        } else {
            None
        };

        if let Some(ch) = mapped {
            output.push(ch);
        }
    }

    output.trim_matches('-').to_string()
}
