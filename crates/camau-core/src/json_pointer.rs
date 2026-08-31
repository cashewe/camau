use std::borrow::Cow;

pub fn valid_identifier(value: &str) -> bool {
    let mut chars = value.chars();
    matches!(chars.next(), Some(first) if first.is_ascii_alphabetic())
        && chars.all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '.' | '_' | '-')
        })
}

pub fn valid_pointer(value: &str, allow_root: bool) -> bool {
    if value.is_empty() {
        return allow_root;
    }
    value.starts_with('/')
        && value.split('/').skip(1).all(|token| {
            let bytes = token.as_bytes();
            let mut index = 0;
            while index < bytes.len() {
                if bytes[index] == b'~' {
                    if index + 1 >= bytes.len() || !matches!(bytes[index + 1], b'0' | b'1') {
                        return false;
                    }
                    index += 2;
                } else {
                    index += 1;
                }
            }
            true
        })
}

pub fn join_pointer(base: &str, token: &str) -> String {
    let escaped = token.replace('~', "~0").replace('/', "~1");
    format!("{base}/{escaped}")
}

pub fn unescape_pointer_token(token: &str) -> Option<Cow<'_, str>> {
    if !token.contains('~') {
        return Some(Cow::Borrowed(token));
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
    Some(Cow::Owned(output))
}
