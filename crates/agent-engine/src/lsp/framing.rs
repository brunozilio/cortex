use std::io::{self, BufRead, Write};

use serde_json::Value;

const MAX_BODY: usize = 64 * 1024 * 1024;

pub fn write_message(out: &mut impl Write, message: &Value) -> io::Result<()> {
    let body = serde_json::to_vec(message)?;
    write!(out, "Content-Length: {}\r\n\r\n", body.len())?;
    out.write_all(&body)?;
    out.flush()
}

pub fn read_message(input: &mut impl BufRead) -> io::Result<Option<Value>> {
    let mut length = None;
    let mut header = String::new();
    loop {
        header.clear();
        if input.read_line(&mut header)? == 0 {
            return Ok(None);
        }
        let line = header.trim_end();
        if line.is_empty() {
            break;
        }
        if let Some((name, value)) = line.split_once(':')
            && name.trim().eq_ignore_ascii_case("content-length")
        {
            length = value.trim().parse::<usize>().ok();
        }
    }
    let length = length
        .filter(|length| *length <= MAX_BODY)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "a message without a valid Content-Length"))?;
    let mut body = vec![0; length];
    input.read_exact(&mut body)?;
    serde_json::from_slice(&body).map(Some).map_err(io::Error::from)
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use serde_json::json;

    use super::*;

    #[test]
    fn a_message_is_framed_by_its_length_in_bytes() {
        let mut out = Vec::new();
        write_message(&mut out, &json!({"id": 1, "text": "ação"})).expect("write");
        let body = r#"{"id":1,"text":"ação"}"#;
        assert_eq!(String::from_utf8(out).expect("utf8"), format!("Content-Length: {}\r\n\r\n{body}", body.len()));
    }

    #[test]
    fn messages_are_read_back_one_by_one_whatever_their_headers() {
        let mut stream = Vec::new();
        write_message(&mut stream, &json!({"id": 1})).expect("write");
        stream.extend_from_slice(
            b"Content-Type: application/vscode-jsonrpc; charset=utf-8\r\ncontent-length: 8\r\n\r\n{\"id\":2}",
        );
        let mut input = Cursor::new(stream);
        assert_eq!(read_message(&mut input).expect("first"), Some(json!({"id": 1})));
        assert_eq!(read_message(&mut input).expect("second"), Some(json!({"id": 2})));
        assert_eq!(read_message(&mut input).expect("end"), None);
    }

    #[test]
    fn a_header_without_a_length_is_an_error() {
        let mut input = Cursor::new(b"X: 1\r\n\r\n{}".to_vec());
        assert!(read_message(&mut input).is_err());
    }
}
