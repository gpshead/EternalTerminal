/// Utility functions for ET

use rand::Rng;
use std::time::{SystemTime, UNIX_EPOCH};

/// Generate a random alphanumeric string of the given length
pub fn gen_random_alphanum(len: usize) -> String {
    const CHARSET: &[u8] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";
    let mut rng = rand::thread_rng();

    (0..len)
        .map(|_| {
            let idx = rng.gen_range(0..CHARSET.len());
            CHARSET[idx] as char
        })
        .collect()
}

/// Get current timestamp in seconds since UNIX epoch
pub fn current_timestamp_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards")
        .as_secs()
}

/// Get current timestamp in milliseconds since UNIX epoch
pub fn current_timestamp_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards")
        .as_millis()
}

/// Split a string by delimiter
pub fn split(s: &str, delim: char) -> Vec<String> {
    s.split(delim).map(|s| s.to_string()).collect()
}

/// Replace first occurrence of `from` with `to` in string
pub fn replace(s: &mut String, from: &str, to: &str) -> bool {
    if let Some(pos) = s.find(from) {
        s.replace_range(pos..pos + from.len(), to);
        true
    } else {
        false
    }
}

/// Replace all occurrences of `from` with `to` in string
pub fn replace_all(s: &mut String, from: &str, to: &str) -> usize {
    if from.is_empty() {
        return 0;
    }

    let mut count = 0;
    let mut start = 0;

    while let Some(pos) = s[start..].find(from) {
        let abs_pos = start + pos;
        s.replace_range(abs_pos..abs_pos + from.len(), to);
        count += 1;
        start = abs_pos + to.len();
    }

    count
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gen_random_alphanum() {
        let s = gen_random_alphanum(10);
        assert_eq!(s.len(), 10);
        assert!(s.chars().all(|c| c.is_alphanumeric()));
    }

    #[test]
    fn test_split() {
        let result = split("a,b,c", ',');
        assert_eq!(result, vec!["a", "b", "c"]);
    }

    #[test]
    fn test_replace() {
        let mut s = "hello world".to_string();
        assert!(replace(&mut s, "world", "rust"));
        assert_eq!(s, "hello rust");

        assert!(!replace(&mut s, "foo", "bar"));
        assert_eq!(s, "hello rust");
    }

    #[test]
    fn test_replace_all() {
        let mut s = "foo bar foo baz".to_string();
        let count = replace_all(&mut s, "foo", "qux");
        assert_eq!(count, 2);
        assert_eq!(s, "qux bar qux baz");
    }
}
