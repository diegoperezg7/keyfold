//! Replaces secret values in a byte stream with `[llavero:NAME]`.
//! Line-buffered so a value split across two reads is still caught.

const MAX_BUFFER: usize = 64 * 1024;

pub struct Redactor {
    secrets: Vec<(Vec<u8>, Vec<u8>)>,
    max_value_len: usize,
    buffer: Vec<u8>,
}

impl Redactor {
    pub fn new(secrets: &[(String, String)]) -> Self {
        let mut list: Vec<(Vec<u8>, Vec<u8>)> = secrets
            .iter()
            .filter(|(_, value)| !value.is_empty())
            .map(|(name, value)| (value.as_bytes().to_vec(), format!("[llavero:{name}]").into_bytes()))
            .collect();
        list.sort_by(|a, b| b.0.len().cmp(&a.0.len()));
        let max_value_len = list.first().map(|s| s.0.len()).unwrap_or(0);
        Self { secrets: list, max_value_len, buffer: Vec::new() }
    }

    /// Returns what can be safely emitted now.
    pub fn feed(&mut self, data: &[u8]) -> Vec<u8> {
        self.buffer.extend_from_slice(data);
        if let Some(newline) = self.buffer.iter().rposition(|&b| b == b'\n') {
            let rest = self.buffer.split_off(newline + 1);
            let ready = std::mem::replace(&mut self.buffer, rest);
            return self.redact(&ready);
        }
        // Long output without newlines: emit all but a tail that could hold a partial value.
        if self.buffer.len() <= MAX_BUFFER {
            return Vec::new();
        }
        let mut redacted = self.redact(&self.buffer);
        let keep = self.max_value_len.saturating_sub(1).min(redacted.len());
        self.buffer = redacted.split_off(redacted.len() - keep);
        redacted
    }

    /// Flushes what is left at end of stream.
    pub fn finish(&mut self) -> Vec<u8> {
        let rest = std::mem::take(&mut self.buffer);
        self.redact(&rest)
    }

    pub fn redact(&self, data: &[u8]) -> Vec<u8> {
        let mut output = data.to_vec();
        for (value, mask) in &self.secrets {
            output = replace_all(&output, value, mask);
        }
        output
    }
}

fn replace_all(haystack: &[u8], needle: &[u8], replacement: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(haystack.len());
    let mut i = 0;
    while i < haystack.len() {
        if haystack[i..].starts_with(needle) {
            out.extend_from_slice(replacement);
            i += needle.len();
        } else {
            out.push(haystack[i]);
            i += 1;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn masks_value_split_across_chunks() {
        let mut r = Redactor::new(&[("EXA_API_KEY".into(), "abc123secret".into())]);
        let mut out = r.feed(b"key=abc12");
        out.extend(r.feed(b"3secret\nnext "));
        out.extend(r.finish());
        assert_eq!(String::from_utf8(out).unwrap(), "key=[llavero:EXA_API_KEY]\nnext ");
    }

    #[test]
    fn masks_value_in_long_output_without_newlines() {
        let value = "zzz-secret-value";
        let mut r = Redactor::new(&[("K".into(), value.into())]);
        let filler = "a".repeat(70 * 1024);
        let mut out = r.feed(format!("{filler}{}", &value[..5]).as_bytes());
        out.extend(r.feed(format!("{}end", &value[5..]).as_bytes()));
        out.extend(r.finish());
        let text = String::from_utf8(out).unwrap();
        assert!(!text.contains(value));
        assert!(text.ends_with("[llavero:K]end"));
    }
}
