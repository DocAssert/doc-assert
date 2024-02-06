use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Path<'a> {
    Root,
    Keys(Vec<Key<'a>>),
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Key<'a> {
    Idx(usize),
    IdxRange(usize, usize),
    Wildcard,
    WildcardArray,
    Field(&'a str),
}

// We cannot implement FromStr for Path because it would confict with timelines
// https://stackoverflow.com/questions/28931515/how-do-i-implement-fromstr-with-a-concrete-lifetime
pub trait JSONPath {
    fn jsonpath(&self) -> Result<Path, Box<dyn std::error::Error>>;
}

impl JSONPath for str {
    fn jsonpath(&self) -> Result<Path, Box<dyn std::error::Error>> {
        Path::from_jsonpath(self)
    }
}

impl<'a> fmt::Display for Key<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Key::Idx(idx) => write!(f, "[{}]", idx),
            Key::Field(key) => write!(f, ".{}", key),
            Key::IdxRange(start, end) => write!(f, "[{}:{}]", start, end),
            Key::Wildcard => write!(f, "*"),
            Key::WildcardArray => write!(f, "[*]"),
        }
    }
}

impl<'a> Path<'a> {
    pub(crate) fn append(&self, next: Key<'a>) -> Path<'a> {
        match self {
            Path::Root => Path::Keys(vec![next]),
            Path::Keys(list) => {
                let mut copy = list.clone();
                copy.push(next);
                Path::Keys(copy)
            }
        }
    }

    pub(crate) fn prefixes(&self, other: &Path) -> bool {
        match (self, other) {
            (Path::Root, Path::Root) => true,
            (Path::Root, Path::Keys(_)) => true,
            (Path::Keys(_), Path::Root) => false,
            (Path::Keys(lhs), Path::Keys(rhs)) => {
                if lhs.len() > rhs.len() {
                    return false;
                }

                lhs.iter().zip(rhs.iter()).all(|(lhs, rhs)| {
                    if lhs == rhs {
                        return true;
                    }
                    if let (Key::Wildcard, Key::Field(_)) = (lhs, rhs) {
                        return true;
                    }
                    if let (Key::WildcardArray, Key::Idx(_)) = (lhs, rhs) {
                        return true;
                    }
                    if let (Key::IdxRange(a, b), Key::Idx(c)) = (lhs, rhs) {
                        return a <= c && c <= b;
                    }
                    false
                })
            }
        }
    }

    pub(crate) fn from_jsonpath(jsonpath: &'a str) -> Result<Self, Box<dyn std::error::Error>> {
        if jsonpath == "$" {
            return Ok(Path::Root);
        }

        let mut keys = Vec::new();

        for segment in jsonpath
            .trim_matches('$')
            .split(|c| c == '.' || c == '[')
            .skip(1)
        {
            keys.push(Self::parse_token(segment)?);
        }

        Ok(Path::Keys(keys))
    }

    fn parse_token(token: &'a str) -> Result<Key<'a>, Box<dyn std::error::Error>> {
        let mut token = token;
        let mut from_array = false;

        if token.ends_with(']') {
            from_array = true;
            token = &token[..token.len() - 1];
        }

        if token == "*" {
            match from_array {
                true => return Ok(Key::WildcardArray),
                false => return Ok(Key::Wildcard),
            }
        }

        if token.contains(':') {
            let mut parts = token.split(':');
            let start: usize = parts.next().unwrap().parse()?;
            let end: usize = parts.next().unwrap().parse()?;
            return Ok(Key::IdxRange(start, end));
        }

        match token.parse::<usize>() {
            Ok(idx) => Ok(Key::Idx(idx)),
            Err(_) => Ok(Key::Field(token)),
        }
    }
}

impl<'a> fmt::Display for Path<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Path::Root => write!(f, "(root)"),
            Path::Keys(keys) => {
                for key in keys {
                    write!(f, "{}", key)?;
                }
                Ok(())
            }
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_parse_jsonpath() {
        let path: Path = "$.a.b.c".jsonpath().unwrap();
        assert_eq!(
            path,
            Path::Keys(vec![Key::Field("a"), Key::Field("b"), Key::Field("c"),])
        );

        let path = "$.a[0].b.c".jsonpath().unwrap();
        assert_eq!(
            path,
            Path::Keys(vec![
                Key::Field("a"),
                Key::Idx(0),
                Key::Field("b"),
                Key::Field("c"),
            ])
        );

        let path = "$.a[0].b[1].c".jsonpath().unwrap();
        assert_eq!(
            path,
            Path::Keys(vec![
                Key::Field("a"),
                Key::Idx(0),
                Key::Field("b"),
                Key::Idx(1),
                Key::Field("c"),
            ])
        );

        let path: Path = "$.a[0].b[1:2].c".jsonpath().unwrap();
        assert_eq!(
            path,
            Path::Keys(vec![
                Key::Field("a"),
                Key::Idx(0),
                Key::Field("b"),
                Key::IdxRange(1, 2),
                Key::Field("c"),
            ])
        );

        let path: Path = "$.a[0].b[*].*.c[0:1]".jsonpath().unwrap();
        assert_eq!(
            path,
            Path::Keys(vec![
                Key::Field("a"),
                Key::Idx(0),
                Key::Field("b"),
                Key::WildcardArray,
                Key::Wildcard,
                Key::Field("c"),
                Key::IdxRange(0, 1),
            ])
        );
    }

    #[test]
    fn test_prefixes() {
        let path1 = "$.a.b.c".jsonpath().unwrap();
        let path2 = "$.a.b.c".jsonpath().unwrap();
        assert!(path1.prefixes(&path2));

        let path1 = "$.a.b".jsonpath().unwrap();
        let path2 = "$.a.b.c".jsonpath().unwrap();
        assert!(path1.prefixes(&path2));

        let path1 = "$.a.b.c".jsonpath().unwrap();
        let path2 = "$.a.b.d".jsonpath().unwrap();
        assert!(!path1.prefixes(&path2));

        let path1 = "$.a.*.c[0:3]".jsonpath().unwrap();
        let path2 = "$.a.b.c[1]".jsonpath().unwrap();
        assert!(path1.prefixes(&path2));

        let path1 = "$.a.*.c[0:3]".jsonpath().unwrap();
        let path2 = "$.a.b.c[3]".jsonpath().unwrap();
        assert!(path1.prefixes(&path2));

        let path1 = "$.a.*.c[0]".jsonpath().unwrap();
        let path2 = "$.a[1].c[0]".jsonpath().unwrap();
        assert!(!path1.prefixes(&path2));

        let path1 = "$.a.*.c[0].*".jsonpath().unwrap();
        let path2 = "$.a.d.c[0]".jsonpath().unwrap();
        assert!(!path1.prefixes(&path2));
    }
}
