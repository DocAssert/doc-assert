// Copyright 2024 The DocAssert Authors
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use regex::Regex;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Path {
    Root,
    Keys(Vec<Key>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Key {
    Idx(usize),
    IdxRange(usize, usize),
    IdxRangeStart(usize),
    IdxRangeEnd(usize),
    Wildcard,
    WildcardArray,
    Field(String),
}

pub(crate) const JSON_PATH_REGEX: &str = r"\$\.?(([a-zA-Z_][a-zA-Z0-9_]*)*(\[\d+\]|\[\d*:\d*\]|(\[\*\]))?)(\.((([a-zA-Z_][a-zA-Z0-9_]*)(\[\d+\]|\[\d*:\d*\]|(\[\*\]))?)|\*))*";
const JSON_PATH_REGEX_FULL: &str = r"^\$\.?(([a-zA-Z_][a-zA-Z0-9_]*)*(\[\d+\]|\[\d*:\d*\]|(\[\*\]))?)(\.((([a-zA-Z_][a-zA-Z0-9_]*)(\[\d+\]|\[\d*:\d*\]|(\[\*\]))?)|\*))*$";

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

impl fmt::Display for Key {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Key::Idx(idx) => write!(f, "[{}]", idx),
            Key::Field(key) => write!(f, ".{}", key),
            Key::IdxRange(start, end) => write!(f, "[{}:{}]", start, end),
            Key::IdxRangeStart(start) => write!(f, "[{}:]", start),
            Key::IdxRangeEnd(end) => write!(f, "[:{}]", end),
            Key::Wildcard => write!(f, "*"),
            Key::WildcardArray => write!(f, "[*]"),
        }
    }
}

impl Path {
    pub(crate) fn append(&self, next: Key) -> Path {
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
            (Path::Keys(expected), Path::Keys(actual)) => {
                if expected.len() > actual.len() {
                    return false;
                }

                expected
                    .iter()
                    .zip(actual.iter())
                    .all(|(expected, actual)| {
                        if expected == actual {
                            return true;
                        }

                        match (expected, actual) {
                            (Key::Wildcard, Key::Field(_)) => true,
                            (Key::WildcardArray, Key::Idx(_)) => true,
                            (Key::IdxRange(a, b), Key::Idx(c)) => a <= c && c < b,
                            (Key::IdxRangeStart(a), Key::Idx(b)) => a <= b,
                            (Key::IdxRangeEnd(a), Key::Idx(b)) => b < a,
                            _ => false,
                        }
                    })
            }
        }
    }

    pub(crate) fn from_jsonpath(jsonpath: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let re = Regex::new(JSON_PATH_REGEX_FULL)?;

        if !re.is_match(jsonpath) {
            return Err("invalid JSONPath".into());
        }

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

    fn parse_token(token: &str) -> Result<Key, Box<dyn std::error::Error>> {
        let mut token = token;
        let mut from_array = false;

        if token.ends_with(']') {
            from_array = true;
            token = &token[..token.len() - 1];
        }

        if token == "*" || token == ":" {
            match from_array {
                true => return Ok(Key::WildcardArray),
                false => return Ok(Key::Wildcard),
            }
        }

        if token.ends_with(':') {
            let start: usize = token.trim_end_matches(':').parse()?;
            return Ok(Key::IdxRangeStart(start));
        }

        if token.starts_with(':') {
            let end: usize = token.trim_start_matches(':').parse()?;
            return Ok(Key::IdxRangeEnd(end));
        }

        if token.contains(':') {
            let mut parts = token.split(':');
            let start: usize = parts.next().unwrap().parse()?;
            let end: usize = parts.next().unwrap().parse()?;
            return Ok(Key::IdxRange(start, end));
        }

        match token.parse::<usize>() {
            Ok(idx) => Ok(Key::Idx(idx)),
            Err(_) => Ok(Key::Field(token.to_owned())),
        }
    }
}

impl fmt::Display for Path {
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
            Path::Keys(vec![
                Key::Field("a".into()),
                Key::Field("b".into()),
                Key::Field("c".into()),
            ])
        );

        let path = "$.a[0].b.c".jsonpath().unwrap();
        assert_eq!(
            path,
            Path::Keys(vec![
                Key::Field("a".into()),
                Key::Idx(0),
                Key::Field("b".into()),
                Key::Field("c".into()),
            ])
        );

        let path = "$.a[0].b[1].c".jsonpath().unwrap();
        assert_eq!(
            path,
            Path::Keys(vec![
                Key::Field("a".into()),
                Key::Idx(0),
                Key::Field("b".into()),
                Key::Idx(1),
                Key::Field("c".into()),
            ])
        );

        let path: Path = "$.a[0].b[1:2].c".jsonpath().unwrap();
        assert_eq!(
            path,
            Path::Keys(vec![
                Key::Field("a".into()),
                Key::Idx(0),
                Key::Field("b".into()),
                Key::IdxRange(1, 2),
                Key::Field("c".into()),
            ])
        );

        let path: Path = "$.a[0].b[*].*.c[0:1]".jsonpath().unwrap();
        assert_eq!(
            path,
            Path::Keys(vec![
                Key::Field("a".into()),
                Key::Idx(0),
                Key::Field("b".into()),
                Key::WildcardArray,
                Key::Wildcard,
                Key::Field("c".into()),
                Key::IdxRange(0, 1),
            ])
        );

        let path: Path = "$[:].a[3:].b[4:].*.c[0:1]".jsonpath().unwrap();
        assert_eq!(
            path,
            Path::Keys(vec![
                Key::WildcardArray,
                Key::Field("a".into()),
                Key::IdxRangeStart(3),
                Key::Field("b".into()),
                Key::IdxRangeStart(4),
                Key::Wildcard,
                Key::Field("c".into()),
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
        assert!(!path1.prefixes(&path2));

        let path1 = "$.a.*.c[0]".jsonpath().unwrap();
        let path2 = "$.a[1].c[0]".jsonpath().unwrap();
        assert!(!path1.prefixes(&path2));

        let path1 = "$.a.*.c[0].*".jsonpath().unwrap();
        let path2 = "$.a.d.c[0]".jsonpath().unwrap();
        assert!(!path1.prefixes(&path2));

        let path1 = "$.a.*.c[:3]".jsonpath().unwrap();
        let path2 = "$.a.d.c[2]".jsonpath().unwrap();
        assert!(path1.prefixes(&path2));

        let path1 = "$.a.*.c[:3]".jsonpath().unwrap();
        let path2 = "$.a.d.c[4]".jsonpath().unwrap();
        assert!(!path1.prefixes(&path2));

        let path1 = "$.a.*.c[3:]".jsonpath().unwrap();
        let path2 = "$.a.d.c[4]".jsonpath().unwrap();
        assert!(path1.prefixes(&path2));
    }

    #[test]
    fn test_prefixes_validation() {
        let path1 = "$.a.b.c".jsonpath();
        assert!(path1.is_ok());

        let path2 = ".a.b.c".jsonpath();
        assert!(path2.is_err());

        let path3 = "$.a.b.c[".jsonpath();
        assert!(path3.is_err());

        let path4 = "$.a.b.c[]".jsonpath();
        assert!(path4.is_err());

        let path5 = "$.a.b.c[1:".jsonpath();
        assert!(path5.is_err());

        let path6 = "$.a.b.c[1:2".jsonpath();
        assert!(path6.is_err());

        let path7 = "$.a.b.c[1:2]".jsonpath();
        assert!(path7.is_ok());

        let path8 = "$[*].a".jsonpath();
        assert!(path8.is_ok());

        let path9 = "id".jsonpath();
        assert!(path9.is_err());

        let path10 = "".jsonpath();
        assert!(path10.is_err());
    }
}
