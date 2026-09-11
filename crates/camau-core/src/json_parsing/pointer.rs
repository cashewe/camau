use std::sync::Arc;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct JsonPointer {
    source: Arc<str>,
    tokens: Arc<[PointerToken]>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PointerToken {
    key: String,
    index: Option<usize>,
}

impl JsonPointer {
    pub fn parse(source: &str) -> Option<Self> {
        let tokens = if source.is_empty() {
            Vec::new()
        } else {
            source
                .strip_prefix('/')?
                .split('/')
                .map(|token| {
                    let key = unescape_pointer_token(token)?;
                    Some(PointerToken {
                        index: canonical_array_index(&key),
                        key,
                    })
                })
                .collect::<Option<Vec<_>>>()?
        };
        Some(Self {
            source: source.into(),
            tokens: tokens.into(),
        })
    }

    pub fn as_str(&self) -> &str {
        &self.source
    }

    pub(crate) fn tokens(&self) -> &[PointerToken] {
        &self.tokens
    }
}

impl PointerToken {
    pub(crate) fn key(&self) -> &str {
        &self.key
    }

    pub(crate) fn index(&self) -> Option<usize> {
        self.index
    }
}

pub fn valid_identifier(value: &str) -> bool {
    let mut chars = value.chars();
    matches!(chars.next(), Some(first) if first.is_ascii_alphabetic())
        && chars.all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '.' | '_' | '-')
        })
}

pub fn valid_pointer(value: &str, allow_root: bool) -> bool {
    (allow_root || !value.is_empty()) && JsonPointer::parse(value).is_some()
}

pub fn join_pointer(base: &str, token: &str) -> String {
    let escaped = token.replace('~', "~0").replace('/', "~1");
    format!("{base}/{escaped}")
}

fn unescape_pointer_token(token: &str) -> Option<String> {
    if !token.contains('~') {
        return Some(token.to_owned());
    }
    let mut output = String::with_capacity(token.len());
    let mut chars = token.chars();
    while let Some(character) = chars.next() {
        if character == '~' {
            match chars.next()? {
                '0' => output.push('~'),
                '1' => output.push('/'),
                _ => return None,
            }
        } else {
            output.push(character);
        }
    }
    Some(output)
}

fn canonical_array_index(token: &str) -> Option<usize> {
    let bytes = token.as_bytes();
    if bytes == b"0" {
        return Some(0);
    }
    if !matches!(bytes.first(), Some(b'1'..=b'9')) || !bytes.iter().all(u8::is_ascii_digit) {
        return None;
    }
    token.parse().ok()
}
