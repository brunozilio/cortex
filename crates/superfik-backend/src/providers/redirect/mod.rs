//! Our side of a browser sign-in (OAuth 2.1 authorization code with PKCE):
//! the verifier's challenge, and the loopback listener the browser is sent
//! back to.

mod loopback;
pub mod pkce;

pub use loopback::Loopback;

fn decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'+' => out.push(b' '),
            b'%' if i + 2 < bytes.len() => {
                match std::str::from_utf8(&bytes[i + 1..i + 3]).ok().and_then(|hex| u8::from_str_radix(hex, 16).ok()) {
                    Some(byte) => {
                        out.push(byte);
                        i += 2;
                    }
                    None => out.push(b'%'),
                }
            }
            byte => out.push(byte),
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn callback(query: &str) -> Result<(String, String), String> {
    let mut code = None;
    let mut state = None;
    let mut error = None;
    for pair in query.split('&') {
        let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
        match key {
            "code" => code = Some(decode(value)),
            "state" => state = Some(decode(value)),
            "error_description" => error = Some(decode(value)),
            "error" if error.is_none() => error = Some(decode(value)),
            _ => {}
        }
    }
    match (code, state, error) {
        (_, _, Some(error)) => Err(error),
        (Some(code), Some(state), None) => Ok((code, state)),
        _ => Err("the sign-in did not return a code".to_owned()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn callbacks_give_the_code_or_the_error() {
        assert_eq!(callback("code=a%2Fb&state=xyz"), Ok(("a/b".to_owned(), "xyz".to_owned())));
        assert_eq!(callback("error=access_denied&state=x"), Err("access_denied".to_owned()));
        assert_eq!(callback("error=e&error_description=User+said+no"), Err("User said no".to_owned()));
        assert!(callback("state=x").is_err());
        assert_eq!(decode("100%"), "100%");
    }
}
