//! Bounded JSONL framing for the pinned backend protocol.
use serde_json::Value;
use std::io::BufRead;

pub const FRAME_LIMIT: usize = 2 * 1024 * 1024;

pub fn read_frame(reader: &mut impl BufRead) -> Result<Option<Value>, String> {
    let mut frame = Vec::new();
    loop {
        let bytes = reader
            .fill_buf()
            .map_err(|_| "Codex transport read failed.")?;
        if bytes.is_empty() {
            return if frame.is_empty() {
                Ok(None)
            } else {
                Err("Codex closed in the middle of a protocol frame.".into())
            };
        }
        let newline = bytes.iter().position(|b| *b == b'\n');
        let count = newline.map_or(bytes.len(), |i| i + 1);
        if frame.len() + count > FRAME_LIMIT {
            return Err("Codex protocol frame exceeds 2 MB.".into());
        }
        frame.extend_from_slice(&bytes[..count]);
        reader.consume(count);
        if newline.is_some() {
            let value: Value = serde_json::from_slice(&frame)
                .map_err(|_| "Codex sent an invalid JSON protocol frame.")?;
            if !value.is_object() {
                return Err("Codex protocol frame must be an object.".into());
            }
            return Ok(Some(value));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufReader, Cursor};
    #[test]
    fn fragmented_utf8_crlf_and_adjacent_frames_keep_their_identity() {
        let bytes = "{\"id\":1,\"result\":\"ᜊ\"}\r\n{\"method\":\"turn/completed\",\"params\":{\"turnId\":\"t\"}}\n";
        let mut reader = BufReader::with_capacity(1, Cursor::new(bytes));
        let first = read_frame(&mut reader).unwrap().unwrap();
        assert_eq!(first["id"], 1);
        assert_eq!(first["result"], "ᜊ");
        assert_eq!(
            read_frame(&mut reader).unwrap().unwrap()["params"]["turnId"],
            "t"
        );
        assert!(read_frame(&mut reader).unwrap().is_none());
    }
    #[test]
    fn oversized_malformed_and_incomplete_frames_fail_without_echoing_payloads() {
        let oversized = vec![b'x'; FRAME_LIMIT + 1];
        for bytes in [
            oversized,
            b"SECRET malformed\n".to_vec(),
            b"{\"id\":1}".to_vec(),
            b"[]\n".to_vec(),
        ] {
            let error =
                read_frame(&mut BufReader::with_capacity(17, Cursor::new(bytes))).unwrap_err();
            assert!(!error.contains("SECRET"));
        }
    }
}
