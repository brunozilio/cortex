use std::ops::Range;

pub(super) fn split(line: &str) -> Vec<Range<usize>> {
    let bytes = line.as_bytes();
    let (mut parts, mut start, mut i) = (Vec::new(), 0, 0);
    let (mut single, mut double) = (false, false);
    while i < bytes.len() {
        let byte = bytes[i];
        match byte {
            b'\\' if !single => i += 1,
            b'\'' if !double => single = !single,
            b'"' if !single => double = !double,
            b'&' if !single && !double && i > 0 && bytes[i - 1] == b'>' => {}
            b'&' | b'|' | b';' | b'\n' if !single && !double => {
                parts.push(start..i);
                let doubled = matches!(byte, b'&' | b'|') && bytes.get(i + 1) == Some(&byte);
                i += usize::from(doubled);
                start = i + 1;
            }
            _ => {}
        }
        i += 1;
    }
    parts.push(start.min(line.len())..line.len());
    parts
}
