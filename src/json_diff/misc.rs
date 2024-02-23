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

pub trait Indent {
    fn indent(&self, level: u32) -> String;
}

impl<T> Indent for T
where
    T: ToString,
{
    fn indent(&self, level: u32) -> String {
        let mut indent = String::new();
        for _ in 0..level {
            indent.push(' ');
        }

        self.to_string()
            .lines()
            .map(|line| format!("{}{}", indent, line))
            .collect::<Vec<_>>()
            .join("\n")
    }
}

pub trait Indexes {
    fn indexes(&self) -> Vec<usize>;
}

impl<T> Indexes for Vec<T> {
    fn indexes(&self) -> Vec<usize> {
        if self.is_empty() {
            vec![]
        } else {
            (0..=self.len() - 1).collect()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_indent() {
        assert_eq!("  foo", "foo".indent(2));
        assert_eq!("  foo\n  bar", "foo\nbar".indent(2));
    }

    #[test]
    fn test_indexes() {
        let empty: Vec<i32> = vec![];
        let empty_indexes: Vec<usize> = vec![];
        assert_eq!(empty.indexes(), empty_indexes);

        assert_eq!(vec!['a', 'b'].indexes(), vec![0, 1]);
    }
}
