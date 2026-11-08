//! Replaces secret values in a byte stream with `[keyfold:NAME]`.
//!
//! Streaming: each `feed` emits everything that can no longer be part of a value and holds
//! back only the tail that could still be the start of one (at most the longest value minus
//! one byte, plus a match that a later value could still overlap). A value split across reads,
//! one spanning several lines (PEM, JSON) or a prompt with no newline ("Password: ") are all
//! handled: nothing waits for a newline, and a prefix of a value is never emitted.
//!
//! Overlapping values are all masked: a value inside a longer one gives the longer mask, two
//! that overlap give both masks, so no byte of either reaches the output.

pub struct Redactor {
    /// (value, mask), values non-empty.
    secrets: Vec<(Vec<u8>, Vec<u8>)>,
    /// Original bytes not emitted yet.
    buffer: Vec<u8>,
}

impl Redactor {
    pub fn new(secrets: &[(String, String)]) -> Self {
        let mut list: Vec<(Vec<u8>, Vec<u8>)> = secrets
            .iter()
            .filter(|(_, value)| !value.is_empty())
            .map(|(name, value)| (value.as_bytes().to_vec(), format!("[keyfold:{name}]").into_bytes()))
            .collect();
        // Longest first: at the same position the longer value wins.
        list.sort_by(|a, b| b.0.len().cmp(&a.0.len()));
        Self { secrets: list, buffer: Vec::new() }
    }

    /// Returns what can be safely emitted now.
    pub fn feed(&mut self, data: &[u8]) -> Vec<u8> {
        self.buffer.extend_from_slice(data);
        let (out, cut) = self.process(&self.buffer, false);
        self.buffer.drain(..cut);
        out
    }

    /// Flushes what is left at end of stream.
    pub fn finish(&mut self) -> Vec<u8> {
        let rest = std::mem::take(&mut self.buffer);
        self.redact(&rest)
    }

    pub fn redact(&self, data: &[u8]) -> Vec<u8> {
        self.process(data, true).0
    }

    /// Redacts `data[..cut]` and returns it with `cut`. At end of stream (`last`) everything is
    /// emitted; otherwise `cut` stops before any tail that could still grow into a value or
    /// into a match overlapping one already found.
    fn process(&self, data: &[u8], last: bool) -> (Vec<u8>, usize) {
        if self.secrets.is_empty() {
            return (data.to_vec(), data.len());
        }
        // Every complete occurrence of every value: (start, end, index of the secret).
        let mut matches = Vec::new();
        for start in 0..data.len() {
            for (index, (value, _)) in self.secrets.iter().enumerate() {
                if data[start..].starts_with(value) {
                    matches.push((start, start + value.len(), index));
                }
            }
        }
        // Longest first at equal starts (the list is already sorted that way).
        matches.sort_by_key(|&(start, end, _)| (start, std::cmp::Reverse(end)));

        let mut cut = data.len();
        if !last {
            // The first position whose remaining bytes are a proper prefix of some value.
            let longest = self.secrets[0].0.len();
            if let Some(hold) = (data.len().saturating_sub(longest)..data.len()).find(|&i| self.could_start_value(&data[i..])) {
                cut = hold;
            }
            // A run of overlapping matches that crosses the cut may still grow: hold it all.
            let mut run_start = None;
            let mut run_end = 0;
            for &(start, end, _) in &matches {
                if run_start.is_none() || start >= run_end {
                    run_start = Some(start);
                    run_end = end;
                } else {
                    run_end = run_end.max(end);
                }
                if let Some(s) = run_start {
                    if s < cut && run_end > cut {
                        cut = s;
                        break;
                    }
                }
            }
        }

        let mut out = Vec::with_capacity(cut);
        let mut i = 0; // next byte not yet emitted or covered
        for &(start, end, index) in &matches {
            if end > cut {
                continue;
            }
            if end <= i {
                continue; // inside a mask already written
            }
            if start > i {
                out.extend_from_slice(&data[i..start]);
            }
            out.extend_from_slice(&self.secrets[index].1);
            i = end;
        }
        if i < cut {
            out.extend_from_slice(&data[i..cut]);
        }
        (out, cut)
    }

    /// `tail` is shorter than some value and matches its start.
    fn could_start_value(&self, tail: &[u8]) -> bool {
        self.secrets.iter().any(|(value, _)| value.len() > tail.len() && value.starts_with(tail))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn redactor(secrets: &[(&str, &str)]) -> Redactor {
        let list: Vec<(String, String)> = secrets.iter().map(|(n, v)| (n.to_string(), v.to_string())).collect();
        Redactor::new(&list)
    }

    /// Feeds `input` cut at `cuts` and returns everything emitted.
    fn run(r: &mut Redactor, input: &[u8], cuts: &[usize]) -> Vec<u8> {
        let mut out = Vec::new();
        let mut from = 0;
        for &to in cuts {
            out.extend(r.feed(&input[from..to]));
            from = to;
        }
        out.extend(r.feed(&input[from..]));
        out.extend(r.finish());
        out
    }

    #[test]
    fn masks_value_split_across_chunks() {
        let mut r = redactor(&[("EXA_API_KEY", "abc123secret")]);
        let mut out = r.feed(b"key=abc12");
        out.extend(r.feed(b"3secret\nnext "));
        out.extend(r.finish());
        assert_eq!(String::from_utf8(out).unwrap(), "key=[keyfold:EXA_API_KEY]\nnext ");
    }

    #[test]
    fn masks_value_in_long_output_without_newlines() {
        let value = "zzz-secret-value";
        let mut r = redactor(&[("K", value)]);
        let filler = "a".repeat(70 * 1024);
        let mut out = r.feed(format!("{filler}{}", &value[..5]).as_bytes());
        out.extend(r.feed(format!("{}end", &value[5..]).as_bytes()));
        out.extend(r.finish());
        let text = String::from_utf8(out).unwrap();
        assert!(!text.contains(value));
        assert!(text.ends_with("[keyfold:K]end"));
    }

    #[test]
    fn multiline_value_is_masked_at_every_split_point() {
        let pem = "-----BEGIN KEY-----\nMIIBVgIBADANBg\nkqhkiG9w0BAQ==\n-----END KEY-----";
        let input = format!("before\n{pem}\nafter");
        let expected = "before\n[keyfold:PEM]\nafter";
        for cut in 0..=input.len() {
            let mut r = redactor(&[("PEM", pem)]);
            let out = run(&mut r, input.as_bytes(), &[cut]);
            assert_eq!(String::from_utf8(out).unwrap(), expected, "split at {cut}");
        }
        // Three pieces, every pair of cut points.
        for a in 0..=input.len() {
            for b in a..=input.len() {
                let mut r = redactor(&[("PEM", pem)]);
                assert_eq!(run(&mut r, input.as_bytes(), &[a, b]), expected.as_bytes(), "split at {a},{b}");
            }
        }
    }

    #[test]
    fn value_without_trailing_newline_is_masked() {
        let mut r = redactor(&[("K", "s3cr3t-value")]);
        let mut out = r.feed(b"token: s3cr3t-");
        out.extend(r.feed(b"value"));
        out.extend(r.finish());
        assert_eq!(out, b"token: [keyfold:K]");
        // A JSON body on one line with no newline, value in the middle.
        let mut r = redactor(&[("K", "s3cr3t-value")]);
        let out = run(&mut r, br#"{"key":"s3cr3t-value","ok":true}"#, &[10]);
        assert_eq!(out, br#"{"key":"[keyfold:K]","ok":true}"#);
    }

    #[test]
    fn a_value_inside_a_longer_one_and_overlapping_values_leak_nothing() {
        let secrets = [("SHORT", "abcd"), ("LONG", "abcdefgh"), ("TAIL", "ghijkl")];
        let cases: &[(&str, &str)] = &[
            ("x abcdefgh y", "x [keyfold:LONG] y"),
            ("x abcd y", "x [keyfold:SHORT] y"),
            ("x abcdefghijkl y", "x [keyfold:LONG][keyfold:TAIL] y"),
            ("x abcdabcdefgh y", "x [keyfold:SHORT][keyfold:LONG] y"),
            ("abcdef", "[keyfold:SHORT]ef"),
        ];
        for (input, expected) in cases {
            for cut in 0..=input.len() {
                let mut r = redactor(&secrets);
                let out = String::from_utf8(run(&mut r, input.as_bytes(), &[cut])).unwrap();
                assert_eq!(&out, expected, "input {input:?} split at {cut}");
            }
            for size in 1..=3 {
                let cuts: Vec<usize> = (size..input.len()).step_by(size).collect();
                let mut r = redactor(&secrets);
                assert_eq!(String::from_utf8(run(&mut r, input.as_bytes(), &cuts)).unwrap(), *expected, "{input:?} in {size}-byte reads");
            }
        }
    }

    #[test]
    fn non_utf8_output_passes_through_and_values_are_still_masked() {
        let mut input = vec![0xff, 0xfe, b'k', b'='];
        input.extend_from_slice(b"secret-1");
        input.extend_from_slice(&[0x80, 0x00, 0xc3]);
        let mut expected = vec![0xff, 0xfe, b'k', b'='];
        expected.extend_from_slice(b"[keyfold:K]");
        expected.extend_from_slice(&[0x80, 0x00, 0xc3]);
        for cut in 0..=input.len() {
            let mut r = redactor(&[("K", "secret-1")]);
            assert_eq!(run(&mut r, &input, &[cut]), expected, "split at {cut}");
        }
    }

    #[test]
    fn prompt_without_newline_is_emitted_at_once() {
        let mut r = redactor(&[("K", "a-long-secret-value-of-forty-bytes-123456")]);
        assert_eq!(r.feed(b"Password: "), b"Password: ");
        // Only a tail that could start the value waits.
        assert_eq!(r.feed(b"a-lo"), b"");
        assert_eq!(r.feed(b"ud"), b"a-loud");
        assert_eq!(r.finish(), b"");
    }

    #[test]
    fn without_secrets_everything_passes_at_once() {
        let mut r = Redactor::new(&[]);
        assert_eq!(r.feed(b"no newline"), b"no newline");
        assert_eq!(r.finish(), b"");
    }
}
