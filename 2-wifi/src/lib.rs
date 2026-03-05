#![cfg_attr(not(test), no_std)]

pub fn contains_token(haystack: &[u8], needle: &[u8]) -> bool {
    if needle.is_empty() || haystack.len() < needle.len() {
        return false;
    }

    haystack
        .windows(needle.len())
        .any(|window| window == needle)
}

#[cfg(test)]
mod tests {
    use super::contains_token;

    #[test]
    fn should_find_ok_in_basic_response() {
        let response = b"\r\nOK\r\n";
        assert!(contains_token(response, b"OK"));
    }

    #[test]
    fn should_find_token_in_longer_payload() {
        let response = b"AT version:1.7.4\r\nSDK version:3.0.0\r\nOK\r\n";
        assert!(contains_token(response, b"SDK version"));
    }

    #[test]
    fn should_return_false_when_token_missing() {
        let response = b"ERROR\r\n";
        assert!(!contains_token(response, b"OK"));
    }

    #[test]
    fn should_return_false_for_empty_needle() {
        let response = b"\r\nOK\r\n";
        assert!(!contains_token(response, b""));
    }
}
