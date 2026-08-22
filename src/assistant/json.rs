//! Flat-JSON parsing for the wire protocol: one object per line, from the
//! input pipe or an HTTP POST. Quote-aware and escape-aware, but deliberately
//! shallow — nested objects are not part of the contract.

// Splits a flat JSON object into (key, value) pairs. Quote-aware, handles
// \" \\ \n \t \r escapes; an array of strings folds into ONE \n-separated
// value (how options travel internally). Deeper nesting is not part of the
// contract.
pub fn json_fields(line: &str) -> Option<Vec<(String, String)>> {
    let inner = line.trim().strip_prefix('{')?.strip_suffix('}')?;
    let mut fields = Vec::new();
    let mut token = String::new();
    let mut tokens: Vec<(String, bool)> = Vec::new(); // (text, came_from_string)
    let mut in_str = false;
    let mut in_arr = false;
    let mut was_str = false;
    let mut esc = false;
    let push = |t: &mut String, tokens: &mut Vec<(String, bool)>, was_str: &mut bool| {
        tokens.push((std::mem::take(t), *was_str));
        *was_str = false;
    };
    for c in inner.chars() {
        if in_str {
            if esc {
                token.push(match c {
                    'n' => '\n',
                    't' => '\t',
                    'r' => '\r',
                    c => c,
                });
                esc = false;
            } else if c == '\\' {
                esc = true;
            } else if c == '"' {
                in_str = false;
                was_str = true;
            } else {
                token.push(c);
            }
        } else if in_arr {
            match c {
                '"' => in_str = true,
                ',' => token.push('\n'),
                ']' => {
                    in_arr = false;
                    was_str = true;
                }
                c if c.is_whitespace() => {}
                c => token.push(c),
            }
        } else {
            match c {
                '"' => in_str = true,
                '[' => in_arr = true,
                ':' | ',' => {
                    push(&mut token, &mut tokens, &mut was_str);
                    tokens.push((c.to_string(), false));
                }
                c if c.is_whitespace() => {}
                c => token.push(c),
            }
        }
    }
    if in_str || in_arr {
        return None;
    }
    push(&mut token, &mut tokens, &mut was_str);

    // tokens now look like: key, ":", value, ",", key, ":", value, …
    let mut it = tokens.into_iter().filter(|(t, s)| *s || !t.is_empty());
    loop {
        let Some((key, _)) = it.next() else { break };
        if key == "," {
            continue;
        }
        if it.next().map(|(t, _)| t) != Some(":".to_string()) {
            return None;
        }
        let (value, _) = it.next()?;
        if value == "," || value == ":" {
            return None;
        }
        fields.push((key, value));
    }
    Some(fields)
}

pub fn json_escape(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\t', "\\t")
        .replace('\r', "\\r")
}

// Looks a key up in the parsed pairs. First match wins.
pub(super) fn get(fields: &[(String, String)], k: &str) -> Option<String> {
    fields.iter().find(|(key, _)| key == k).map(|(_, v)| v.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_fields_handles_colons_commas_and_escapes_inside_strings() {
        let fields = json_fields(r#"{"message":"deploy: ok, \"prod\"","from":"ci","progress":62}"#).unwrap();
        assert_eq!(fields[0], ("message".to_string(), r#"deploy: ok, "prod""#.to_string()));
        assert_eq!(fields[1], ("from".to_string(), "ci".to_string()));
        assert_eq!(fields[2], ("progress".to_string(), "62".to_string()));
    }

    #[test]
    fn json_fields_folds_string_arrays_into_one_value() {
        let fields = json_fields(r#"{"actions":["sim","não, depois","com \"aspas\""],"from":"x"}"#).unwrap();
        assert_eq!(fields[0], ("actions".to_string(), "sim\nnão, depois\ncom \"aspas\"".to_string()));
        assert_eq!(fields[1], ("from".to_string(), "x".to_string()));
        // empty array → empty value (parse falls back to default options)
        assert_eq!(json_fields(r#"{"actions":[]}"#).unwrap()[0].1, "");
        // unterminated array is invalid
        assert!(json_fields(r#"{"actions":["a","b"}"#).is_none());
    }

    #[test]
    fn escape_round_trips_newlines_tabs_quotes_and_backslashes() {
        let text = "rodar:\n\tnpm test \"tudo\" \\o/\r";
        let line = format!("{{\"message\":\"{}\"}}", json_escape(text));
        let fields = json_fields(&line).unwrap();
        assert_eq!(fields[0], ("message".to_string(), text.to_string()));
    }
}
