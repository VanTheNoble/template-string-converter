use std::collections::HashMap;
use std::io::{self, BufRead, Write};
use serde_json::{json, Value};

macro_rules! log {
    ($($arg:tt)*) => {
        eprintln!("[template-string-converter] {}", format!($($arg)*));
    };
}

fn read_message(reader: &mut impl BufRead) -> Option<Value> {
    let mut content_length = 0usize;
    loop {
        let mut line = String::new();
        reader.read_line(&mut line).ok()?;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            break;
        }
        if let Some(rest) = trimmed.strip_prefix("Content-Length: ") {
            content_length = rest.parse().ok()?;
        }
    }
    let mut buf = vec![0u8; content_length];
    reader.read_exact(&mut buf).ok()?;
    serde_json::from_slice(&buf).ok()
}

fn write_message(writer: &mut impl Write, msg: &Value) {
    let body = msg.to_string();
    write!(writer, "Content-Length: {}\r\n\r\n{}", body.len(), body).ok();
    writer.flush().ok();
}

fn offset_to_position(text: &str, target: usize) -> (u32, u32) {
    let mut line = 0u32;
    let mut col = 0u32;
    for (i, ch) in text.char_indices() {
        if i == target {
            return (line, col);
        }
        if ch == '\n' {
            line += 1;
            col = 0;
        } else {
            col += ch.len_utf16() as u32;
        }
    }
    (line, col)
}

fn count_preceding_backslashes(bytes: &[u8], pos: usize) -> usize {
    let mut count = 0;
    let mut j = pos as isize - 1;
    while j >= 0 && bytes[j as usize] == b'\\' {
        count += 1;
        j -= 1;
    }
    count
}

fn find_enclosing_quotes(text: &str, dollar_pos: usize) -> Option<(usize, usize)> {
    let bytes = text.as_bytes();

    let mut open_pos: Option<usize> = None;
    let mut i = (dollar_pos as isize) - 1;
    while i >= 0 {
        let idx = i as usize;
        match bytes[idx] {
            b'\'' | b'"' => {
                let escapes = count_preceding_backslashes(bytes, idx);
                if escapes % 2 == 0 {
                    open_pos = Some(idx);
                    break;
                }
            }
            b'`' => return None,
            b'\n' => return None,
            _ => {}
        }
        i -= 1;
    }

    let open_pos = open_pos?;
    let quote = bytes[open_pos];

    let mut k = dollar_pos;
    while k < bytes.len() {
        match bytes[k] {
            c if c == quote => {
                let escapes = count_preceding_backslashes(bytes, k);
                if escapes % 2 == 0 {
                    return Some((open_pos, k));
                }
            }
            b'\n' => return None,
            _ => {}
        }
        k += 1;
    }

    None
}

// Find the byte range [start, end) that differs between old and new.
fn changed_range(old: &str, new: &str) -> (usize, usize) {
    let old_bytes = old.as_bytes();
    let new_bytes = new.as_bytes();
    let min_len = old_bytes.len().min(new_bytes.len());

    let mut start = 0;
    while start < min_len && old_bytes[start] == new_bytes[start] {
        start += 1;
    }

    let mut suffix = 0;
    while suffix < min_len - start
        && old_bytes[old_bytes.len() - 1 - suffix] == new_bytes[new_bytes.len() - 1 - suffix]
    {
        suffix += 1;
    }

    let end = new_bytes.len() - suffix;
    (start, end)
}

fn main() {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut reader = io::BufReader::new(stdin.lock());
    let mut writer = io::BufWriter::new(stdout.lock());
    let mut documents: HashMap<String, String> = HashMap::new();
    let mut next_request_id = 1i32;
    // Content before our last edit, per URI. Used to detect undo.
    let mut pre_edit_content: HashMap<String, String> = HashMap::new();

    log!("server started");

    loop {
        let msg = match read_message(&mut reader) {
            Some(m) => m,
            None => {
                log!("stdin closed, exiting");
                break;
            }
        };

        let method = msg["method"].as_str().unwrap_or("");
        let id = msg.get("id").cloned();

        log!("received: {}", method);

        match method {
            "initialize" => {
                write_message(&mut writer, &json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": {
                        "capabilities": {
                            "textDocumentSync": {
                                "openClose": true,
                                "change": 1
                            }
                        },
                        "serverInfo": {
                            "name": "template-string-converter",
                            "version": "0.0.2"
                        }
                    }
                }));
            }

            "initialized" => {}

            "textDocument/didOpen" => {
                if let (Some(uri), Some(text)) = (
                    msg["params"]["textDocument"]["uri"].as_str(),
                    msg["params"]["textDocument"]["text"].as_str(),
                ) {
                    log!("didOpen: {} (len={})", uri, text.len());
                    documents.insert(uri.to_string(), text.to_string());
                }
            }

            "textDocument/didChange" => {
                if let Some(uri) = msg["params"]["textDocument"]["uri"].as_str() {
                    if let Some(changes) = msg["params"]["contentChanges"].as_array() {
                        if let Some(last) = changes.last() {
                            if let Some(new_text) = last["text"].as_str() {
                                let uri = uri.to_string();
                                let new_text = new_text.to_string();
                                log!("didChange: {} (len={})", uri, new_text.len());

                                // Check if this is an undo of our edit
                                if let Some(before) = pre_edit_content.get(&uri) {
                                    if *before == new_text {
                                        log!("  undo detected, skipping");
                                        pre_edit_content.remove(&uri);
                                        documents.insert(uri, new_text);
                                        continue;
                                    }
                                }

                                let old_text = documents.get(&uri).cloned().unwrap_or_default();
                                let (range_start, range_end) = changed_range(&old_text, &new_text);

                                log!("  changed range: {}..{}", range_start, range_end);

                                // Only scan the changed region for ${
                                // Scan one byte early to catch a '$' that precedes the
                                // change, and one byte late to catch a '{' that follows it
                                // (e.g. typing '$' in front of an existing '{').
                                let bytes = new_text.as_bytes();
                                let scan_start = range_start.saturating_sub(1);
                                let scan_end = (range_end + 1).min(bytes.len());
                                // (open_quote, close_quote, dollar_pos)
                                let mut found: Vec<(usize, usize, usize)> = Vec::new();
                                let mut i = scan_start;
                                while i + 1 < scan_end {
                                    if bytes[i] == b'$' && bytes[i + 1] == b'{' {
                                        if let Some((open, close)) = find_enclosing_quotes(&new_text, i) {
                                            log!("  found ${{...}} at {} inside quotes {}..{}", i, open, close);
                                            found.push((open, close, i));
                                            i = close + 1;
                                            continue;
                                        }
                                    }
                                    i += 1;
                                }

                                documents.insert(uri.clone(), new_text.clone());

                                if !found.is_empty() {
                                    // Save content before edit for undo detection
                                    pre_edit_content.insert(uri.clone(), new_text.clone());

                                    let mut edits = Vec::new();
                                    for (open, close, dollar) in &found {
                                        let (ol, oc) = offset_to_position(&new_text, *open);
                                        let (cl, cc) = offset_to_position(&new_text, *close);
                                        edits.push(json!({
                                            "range": {
                                                "start": {"line": ol, "character": oc},
                                                "end":   {"line": ol, "character": oc + 1}
                                            },
                                            "newText": "`"
                                        }));
                                        edits.push(json!({
                                            "range": {
                                                "start": {"line": cl, "character": cc},
                                                "end":   {"line": cl, "character": cc + 1}
                                            },
                                            "newText": "`"
                                        }));

                                        // Insert the closing '}' ourselves when it is not
                                        // already there. Zed only auto-closes '{' when the
                                        // following char is in `autoclose_before` (not word
                                        // chars), so we can't rely on it. The '${' spans
                                        // bytes [dollar, dollar+2); the slot right after is
                                        // dollar+2.
                                        let brace_slot = dollar + 2;
                                        if bytes.get(brace_slot) != Some(&b'}') {
                                            let (bl, bc) = offset_to_position(&new_text, brace_slot);
                                            edits.push(json!({
                                                "range": {
                                                    "start": {"line": bl, "character": bc},
                                                    "end":   {"line": bl, "character": bc}
                                                },
                                                "newText": "}"
                                            }));
                                        }
                                    }

                                    let req_id = next_request_id;
                                    next_request_id += 1;

                                    log!("  sending workspace/applyEdit (id={})", req_id);
                                    write_message(&mut writer, &json!({
                                        "jsonrpc": "2.0",
                                        "id": req_id,
                                        "method": "workspace/applyEdit",
                                        "params": {
                                            "label": "Convert quotes to template literal",
                                            "edit": {
                                                "changes": { uri.as_str(): edits }
                                            }
                                        }
                                    }));
                                }
                            }
                        }
                    }
                }
            }

            "shutdown" => {
                write_message(&mut writer, &json!({"jsonrpc": "2.0", "id": id, "result": null}));
            }

            "exit" => break,

            _ => {
                if id.is_some() && !method.is_empty() {
                    write_message(&mut writer, &json!({"jsonrpc": "2.0", "id": id, "result": null}));
                }
            }
        }
    }
}
