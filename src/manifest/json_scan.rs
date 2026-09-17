use std::ops::Range;

use super::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Segment<'a> {
    Key(&'a str),
    Index(usize),
}

fn parse_path<'a>(path: &'a str) -> Vec<Segment<'a>> {
    path.split('.')
        .map(|s| s.parse::<usize>().map(Segment::Index).unwrap_or(Segment::Key(s)))
        .collect()
}

pub fn find_string_value(content: &str, path: &str) -> Result<Range<usize>, Error> {
    let _: serde_json::Value = serde_json::from_str(content).map_err(|e| Error::Parse {
        kind: "json",
        detail: e.to_string(),
    })?;
    match value(&parse_path(path), path, content.as_bytes(), 0)? {
        Some(r) => Ok(r),
        None => Err(Error::FieldNotFound(path.into())),
    }
}

fn value(target: &[Segment], path: &str, bytes: &[u8], mut i: usize) -> Result<Option<Range<usize>>, Error> {
    i = skip_ws(bytes, i);
    if target.is_empty() {
        return if bytes.get(i) == Some(&b'"') {
            let start = i;
            Ok(Some(start..string(bytes, i)))
        } else {
            Err(Error::NotAString(path.into()))
        };
    }
    match bytes.get(i) {
        Some(b'{') => object(target, path, bytes, i),
        Some(b'[') => array(target, path, bytes, i),
        Some(b'"') => {
            string(bytes, i);
            Ok(None)
        }
        _ => {
            scalar(bytes, i);
            Ok(None)
        }
    }
}

fn object(target: &[Segment], path: &str, bytes: &[u8], mut i: usize) -> Result<Option<Range<usize>>, Error> {
    i += 1;
    loop {
        i = skip_ws(bytes, i);
        if bytes.get(i) == Some(&b'}') {
            return Ok(None);
        }
        let key_start = i;
        if bytes.get(i) != Some(&b'"') {
            return Ok(None);
        }
        i = string(bytes, i);
        let key: String =
            serde_json::from_str(std::str::from_utf8(&bytes[key_start..i]).map_err(|e| Error::Parse {
                kind: "json",
                detail: e.to_string(),
            })?)
            .map_err(|e| Error::Parse {
                kind: "json",
                detail: e.to_string(),
            })?;
        i = skip_ws(bytes, i);
        if bytes.get(i) == Some(&b':') {
            i += 1;
        }
        i = skip_ws(bytes, i);
        if matches!(target.first(), Some(Segment::Key(k)) if *k == key) {
            if let Some(r) = value(&target[1..], path, bytes, i)? {
                return Ok(Some(r));
            }
            return Err(Error::FieldNotFound(path.into()));
        }
        i = consume_value(bytes, i);
        i = skip_ws(bytes, i);
        match bytes.get(i) {
            Some(b',') => {
                i += 1;
                continue;
            }
            Some(b'}') => return Ok(None),
            _ => return Ok(None),
        }
    }
}

fn array(target: &[Segment], path: &str, bytes: &[u8], mut i: usize) -> Result<Option<Range<usize>>, Error> {
    i += 1;
    i = skip_ws(bytes, i);
    if bytes.get(i) == Some(&b']') {
        return Ok(None);
    }
    let mut idx = 0usize;
    loop {
        if matches!(target.first(), Some(Segment::Index(n)) if *n == idx) {
            if let Some(r) = value(&target[1..], path, bytes, i)? {
                return Ok(Some(r));
            }
            return Err(Error::FieldNotFound(path.into()));
        }
        i = consume_value(bytes, i);
        i = skip_ws(bytes, i);
        match bytes.get(i) {
            Some(b',') => {
                i += 1;
                idx += 1;
                continue;
            }
            Some(b']') => return Ok(None),
            _ => return Ok(None),
        }
    }
}

fn string(bytes: &[u8], mut i: usize) -> usize {
    i += 1;
    while i < bytes.len() {
        match bytes[i] {
            b'"' => return i + 1,
            b'\\' => {
                if bytes.get(i + 1) == Some(&b'u') {
                    i += 6;
                } else {
                    i += 2;
                }
            }
            _ => i += 1,
        }
    }
    i
}

fn scalar(bytes: &[u8], mut i: usize) -> usize {
    while i < bytes.len() && !matches!(bytes[i], b',' | b'}' | b']' | b' ' | b'\t' | b'\n' | b'\r') {
        i += 1;
    }
    i
}

fn skip_ws(bytes: &[u8], mut i: usize) -> usize {
    while i < bytes.len() && matches!(bytes[i], b' ' | b'\t' | b'\n' | b'\r') {
        i += 1;
    }
    i
}

fn consume_value(bytes: &[u8], i: usize) -> usize {
    let i = skip_ws(bytes, i);
    match bytes.get(i) {
        Some(b'"') => string(bytes, i),
        Some(b'{' | b'[') => skip_container(bytes, i),
        _ => scalar(bytes, i),
    }
}

fn skip_container(bytes: &[u8], mut i: usize) -> usize {
    let close = if bytes[i] == b'{' { b'}' } else { b']' };
    i += 1;
    loop {
        i = skip_ws(bytes, i);
        if bytes.get(i) == Some(&close) {
            return i + 1;
        }
        if close == b'}' {
            if bytes.get(i) == Some(&b'"') {
                i = string(bytes, i);
            }
            i = skip_ws(bytes, i);
            if bytes.get(i) == Some(&b':') {
                i += 1;
            }
            i = skip_ws(bytes, i);
        }
        i = consume_value(bytes, i);
        i = skip_ws(bytes, i);
        match bytes.get(i) {
            Some(b',') => {
                i += 1;
                continue;
            }
            Some(&c) if c == close => return i + 1,
            _ => return i,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Error, find_string_value};

    fn span_of(content: &str, path: &str) -> String {
        content[find_string_value(content, path).unwrap()].to_string()
    }

    #[test]
    fn top_level_key() {
        assert_eq!(span_of(r#"{"version":"1.0.0"}"#, "version"), "\"1.0.0\"");
    }

    #[test]
    fn nested_path() {
        let doc = r#"{"project":{"version":"0.5.1","name":"x"}}"#;
        assert_eq!(span_of(doc, "project.version"), "\"0.5.1\"");
    }

    #[test]
    fn key_inside_array_of_objects() {
        let doc = r#"{"items":[{"name":"a","version":"1.0"},{"name":"b","version":"2.0"}]}"#;
        assert_eq!(span_of(doc, "items.1.version"), "\"2.0\"");
    }

    #[test]
    fn sibling_branch_same_key() {
        let doc = r#"{"a":{"version":"wrong"},"b":{"version":"right"}}"#;
        assert_eq!(span_of(doc, "b.version"), "\"right\"");
    }

    #[test]
    fn escaped_characters_nearby() {
        let doc = r#"{"name":"tab\there","version":"3.0.0","quote":"say \"hi\""}"#;
        assert_eq!(span_of(doc, "version"), "\"3.0.0\"");
    }

    #[test]
    fn indentation_and_tabs() {
        let doc = "{\n    \"alpha\": 1,\n\t\"beta\":\"x\",\n    \"version\": \"1.2.3\"\n}\n";
        assert_eq!(span_of(doc, "version"), "\"1.2.3\"");
    }

    #[test]
    fn missing_path_errors() {
        assert!(matches!(
            find_string_value(r#"{"version":"1.0.0"}"#, "project.version").unwrap_err(),
            Error::FieldNotFound(_)
        ));
    }

    #[test]
    fn target_not_a_string_errors() {
        assert!(matches!(
            find_string_value(r#"{"version":123}"#, "version").unwrap_err(),
            Error::NotAString(_)
        ));
    }

    #[test]
    fn invalid_json_errors() {
        assert!(matches!(
            find_string_value(r#"{"version":"1.0.0""#, "version").unwrap_err(),
            Error::Parse { kind: "json", .. }
        ));
    }
}
