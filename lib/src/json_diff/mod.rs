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

mod misc;
pub mod path;

use misc::{Indent, Indexes};
use path::{JSONPath, Key, Path};
use serde_json::Value;
use std::{collections::HashSet, fmt, hash::Hash};

/// Mode for how JSON values should be compared.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub(crate) enum CompareMode {
    /// The two JSON values don't have to be exactly equal. The "expected" value is only required to
    /// be "contained" inside "actual". See [crate documentation](index.html) for examples.
    ///
    /// The mode used with [`assert_json_include`].
    Inclusive,
    /// The two JSON values must be exactly equal.
    ///
    /// The mode used with [`assert_json_eq`].
    Strict,
}

/// How should numbers be compared.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub(crate) enum NumericMode {
    /// Different numeric types aren't considered equal.
    Strict,
    /// All numeric types are converted to float before comparison.
    AssumeFloat,
}

/// Configuration for how JSON values should be compared.
#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(missing_copy_implementations)]
pub(crate) struct Config {
    pub(crate) compare_mode: CompareMode,
    pub(crate) numeric_mode: NumericMode,
    pub(crate) ignore_paths: Vec<Path>,
    pub(crate) ignore_orders: Vec<Path>,
}

impl Config {
    /// Create a new [`Config`] using the given [`CompareMode`].
    ///
    /// The default `numeric_mode` is be [`NumericMode::Strict`].
    pub fn new(compare_mode: CompareMode) -> Self {
        Self {
            compare_mode,
            numeric_mode: NumericMode::Strict,
            ignore_paths: vec![],
            ignore_orders: vec![],
        }
    }

    /// Change the config's numeric mode.
    ///
    /// The default `numeric_mode` is be [`NumericMode::Strict`].
    pub fn numeric_mode(mut self, numeric_mode: NumericMode) -> Self {
        self.numeric_mode = numeric_mode;
        self
    }

    /// Change the config's compare mode.
    pub fn compare_mode(mut self, compare_mode: CompareMode) -> Self {
        self.compare_mode = compare_mode;
        self
    }

    /// Add a path to the list of paths to ignore.
    pub fn ignore_path(mut self, path: Path) -> Self {
        self.ignore_paths.push(path);
        self
    }

    /// Checks if the given path should be ignored.
    pub fn to_ignore(&self, path: &Path) -> bool {
        self.ignore_paths.iter().any(|p| p.prefixes(path))
    }

    /// Add a path to the list of paths to ignore order.
    /// This is only used when comparing arrays.
    pub fn ignore_order(mut self, path: Path) -> Self {
        self.ignore_orders.push(path);
        self
    }

    /// Checks if the given path should be ignored order.
    pub fn to_ignore_order(&self, path: &Path) -> bool {
        self.ignore_orders.iter().any(|p| p == path)
    }
}

pub(crate) fn diff<'a>(
    expected: &'a Value,
    actual: &'a Value,
    config: Config,
) -> Vec<Difference<'a>> {
    let mut acc = Accumulator::collector();

    diff_with(expected, actual, &config, Path::Root, &mut acc);

    acc.into_vec()
}

fn diff_with<'a>(
    expected: &'a Value,
    actual: &'a Value,
    config: &Config,
    path: Path,
    acc: &mut Accumulator<'a>,
) {
    let mut folder = DiffFolder {
        actual,
        path,
        acc,
        config,
    };

    fold_json(expected, &mut folder);
}
#[derive(Debug)]
struct DiffFolder<'a, 'b> {
    actual: &'a Value,
    path: Path,
    acc: &'b mut Accumulator<'a>,
    config: &'b Config,
}

macro_rules! accumulate {
    ($self:expr, $expected:expr, $actual:expr) => {
        $self.acc.accumulate(
            &$self.config,
            &$self.path,
            Difference {
                expected: $expected,
                actual: $actual,
                path: $self.path.clone(),
                compare_mode: $self.config.compare_mode,
            },
        );

        if let Accumulator::Flag(true) = $self.acc {
            return;
        }
    };
}

macro_rules! direct_compare {
    ($name:ident) => {
        fn $name(&mut self, expected: &'a Value) {
            if self.actual != expected {
                self.acc.accumulate(
                    &self.config,
                    &self.path,
                    Difference {
                        expected: Some(expected),
                        actual: Some(&self.actual),
                        path: self.path.clone(),
                        compare_mode: self.config.compare_mode,
                    },
                );

                if let Accumulator::Flag(true) = self.acc {
                    return;
                }
            }
        }
    };
}

impl<'a, 'b> DiffFolder<'a, 'b> {
    direct_compare!(on_null);
    direct_compare!(on_bool);
    direct_compare!(on_string);

    fn on_number(&mut self, expected: &'a Value) {
        let is_equal = match self.config.numeric_mode {
            NumericMode::Strict => self.actual == expected,
            NumericMode::AssumeFloat => self.actual.as_f64() == expected.as_f64(),
        };

        if !is_equal {
            accumulate!(self, Some(expected), Some(self.actual));
        }
    }

    fn on_array(&mut self, expected: &'a Value) {
        if let Some(actual) = self.actual.as_array() {
            let expected = expected.as_array().unwrap();

            match self.config.compare_mode {
                CompareMode::Inclusive => {
                    for (idx, actual) in actual.iter().enumerate() {
                        if let Accumulator::Flag(true) = self.acc {
                            return;
                        }

                        let path = self.path.append(Key::Idx(idx));

                        if let Some(expected) = expected.get(idx) {
                            diff_with(expected, actual, self.config, path, self.acc)
                        } else {
                            accumulate!(self, None, Some(self.actual));
                        }
                    }
                }
                CompareMode::Strict => {
                    let all_keys = actual
                        .indexes()
                        .into_iter()
                        .chain(expected.indexes())
                        .collect::<HashSet<_>>();
                    for key in all_keys {
                        if let Accumulator::Flag(true) = self.acc {
                            return;
                        }

                        let path = self.path.append(Key::Idx(key));

                        match (expected.get(key), actual.get(key)) {
                            (Some(expected), Some(actual)) => {
                                diff_with(expected, actual, self.config, path, self.acc);
                            }
                            (None, Some(actual)) => {
                                accumulate!(self, None, Some(actual));
                            }
                            (Some(expected), None) => {
                                accumulate!(self, Some(expected), None);
                            }
                            (None, None) => {
                                unreachable!("at least one of the maps should have the key")
                            }
                        }
                    }
                }
            }
        } else {
            accumulate!(self, Some(expected), Some(self.actual));
        }
    }

    fn on_array_unordered(&mut self, expected_json: &'a Value) {
        if let Some(actual) = self.actual.as_array() {
            let expected = expected_json.as_array().unwrap();
            if actual.len() > expected.len() {
                accumulate!(self, Some(expected_json), Some(self.actual));
            }

            if actual.len() != expected.len() && self.config.compare_mode == CompareMode::Strict {
                accumulate!(self, Some(expected_json), Some(self.actual));
            }

            let mut visited_keys: HashSet<usize> = HashSet::new();

            for (idx, value) in actual.iter().enumerate() {
                let mut found = false;
                let path = self.path.append(Key::Idx(idx));

                for (expected_idx, expected_value) in expected.iter().enumerate() {
                    if visited_keys.contains(&expected_idx) {
                        continue;
                    }

                    let mut acc = Accumulator::flag();

                    diff_with(expected_value, value, self.config, path.clone(), &mut acc);

                    if !acc.has_diff() {
                        visited_keys.insert(expected_idx);
                        found = true;
                        break;
                    }
                }

                if !found {
                    accumulate!(self, Some(expected_json), Some(self.actual));
                }
            }

            if visited_keys.len() != expected.len() {
                accumulate!(self, Some(expected_json), Some(self.actual));
            }
        }
    }

    fn on_object(&mut self, expected: &'a Value) {
        if let Some(actual) = self.actual.as_object() {
            let expected = expected.as_object().unwrap();

            match self.config.compare_mode {
                CompareMode::Inclusive => {
                    for (key, actual) in actual.iter() {
                        if let Accumulator::Flag(true) = self.acc {
                            return;
                        }

                        let path = self.path.append(Key::Field(key.clone()));

                        if let Some(expected) = expected.get(key) {
                            diff_with(expected, actual, self.config, path, self.acc)
                        } else {
                            accumulate!(self, None, Some(self.actual));
                        }
                    }
                }
                CompareMode::Strict => {
                    let all_keys = actual.keys().chain(expected.keys()).collect::<HashSet<_>>();
                    for key in all_keys {
                        if let Accumulator::Flag(true) = self.acc {
                            return;
                        }

                        let path = self.path.append(Key::Field(key.clone()));

                        match (expected.get(key), actual.get(key)) {
                            (Some(expected), Some(actual)) => {
                                diff_with(expected, actual, self.config, path, self.acc);
                            }
                            (None, Some(actual)) => {
                                accumulate!(self, None, Some(actual));
                            }
                            (Some(expected), None) => {
                                accumulate!(self, Some(expected), None);
                            }
                            (None, None) => {
                                unreachable!("at least one of the maps should have the key")
                            }
                        }
                    }
                }
            }
        } else {
            accumulate!(self, Some(expected), Some(self.actual));
        }
    }
}

#[derive(Debug)]
enum Accumulator<'a> {
    Vec(Vec<Difference<'a>>),
    Flag(bool),
}

impl<'a> Accumulator<'a> {
    fn collector() -> Self {
        Accumulator::Vec(vec![])
    }

    fn flag() -> Self {
        Accumulator::Flag(false)
    }

    fn accumulate(&mut self, config: &Config, path: &Path, diff: Difference<'a>) -> bool {
        match self {
            Accumulator::Vec(vec) => {
                if config.to_ignore(path) {
                    return true;
                }

                vec.push(diff);

                false
            }
            Accumulator::Flag(value) => {
                if config.to_ignore(path) {
                    return true;
                }

                if !*value {
                    *value = true;
                }

                true
            }
        }
    }

    fn has_diff(&self) -> bool {
        match self {
            Accumulator::Vec(vec) => !vec.is_empty(),
            Accumulator::Flag(value) => *value,
        }
    }

    fn into_vec(self) -> Vec<Difference<'a>> {
        match self {
            Accumulator::Vec(vec) => vec,
            Accumulator::Flag(_) => vec![],
        }
    }
}

#[derive(Debug, PartialEq)]
pub(crate) struct Difference<'a> {
    path: Path,
    expected: Option<&'a Value>,
    actual: Option<&'a Value>,
    compare_mode: CompareMode,
}

impl<'a> fmt::Display for Difference<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let json_to_string = |json: &Value| serde_json::to_string_pretty(json).unwrap();

        match (&self.compare_mode, &self.expected, &self.actual) {
            (CompareMode::Inclusive, Some(expected), Some(actual)) => {
                writeln!(f, "json atoms at path \"{}\" are not equal:", self.path)?;
                writeln!(f, "    actual:")?;
                writeln!(f, "{}", json_to_string(actual).indent(8))?;
                writeln!(f, "    expected:")?;
                write!(f, "{}", json_to_string(expected).indent(8))?;
            }
            (CompareMode::Inclusive, None, Some(_actual)) => {
                write!(
                    f,
                    "json atom at path \"{}\" is missing from expected",
                    self.path
                )?;
            }
            (CompareMode::Inclusive, Some(_expected), None) => {
                unreachable!("stuff missing expected wont produce an error")
            }
            (CompareMode::Inclusive, None, None) => unreachable!("can't both be missing"),

            (CompareMode::Strict, Some(expected), Some(actual)) => {
                writeln!(f, "json atoms at path \"{}\" are not equal:", self.path)?;
                writeln!(f, "    actual:")?;
                writeln!(f, "{}", json_to_string(expected).indent(8))?;
                writeln!(f, "    expected:")?;
                write!(f, "{}", json_to_string(actual).indent(8))?;
            }
            (CompareMode::Strict, None, Some(_)) => {
                write!(
                    f,
                    "json atom at path \"{}\" is missing from expected",
                    self.path
                )?;
            }
            (CompareMode::Strict, Some(_), None) => {
                write!(
                    f,
                    "json atom at path \"{}\" is missing from actual",
                    self.path
                )?;
            }
            (CompareMode::Strict, None, None) => unreachable!("can't both be missing"),
        }

        Ok(())
    }
}

fn fold_json<'a>(json: &'a Value, folder: &mut DiffFolder<'a, '_>) {
    match json {
        Value::Null => folder.on_null(json),
        Value::Bool(_) => folder.on_bool(json),
        Value::Number(_) => folder.on_number(json),
        Value::String(_) => folder.on_string(json),
        Value::Array(_) => {
            if folder.config.to_ignore_order(&folder.path) {
                folder.on_array_unordered(json)
            } else {
                folder.on_array(json)
            }
        }
        Value::Object(_) => folder.on_object(json),
    }
}

#[cfg(test)]
mod test {
    #[allow(unused_imports)]
    use super::*;
    use serde_json::{json, Result, Value};
    use std::fs;

    fn load_json_from_file(file_path: &str) -> Result<Value> {
        let data = fs::read_to_string(file_path).expect("Unable to read file");
        serde_json::from_str(&data)
    }

    #[test]
    fn test_diffing_leaf_json() {
        let diffs = diff(
            &json!(null),
            &json!(null),
            Config::new(CompareMode::Inclusive),
        );
        assert_eq!(diffs, vec![]);

        let diffs = diff(
            &json!(false),
            &json!(false),
            Config::new(CompareMode::Inclusive),
        );
        assert_eq!(diffs, vec![]);

        let diffs = diff(
            &json!(true),
            &json!(true),
            Config::new(CompareMode::Inclusive),
        );
        assert_eq!(diffs, vec![]);

        let diffs = diff(
            &json!(false),
            &json!(true),
            Config::new(CompareMode::Inclusive),
        );
        assert_eq!(diffs.len(), 1);

        let diffs = diff(
            &json!(true),
            &json!(false),
            Config::new(CompareMode::Inclusive),
        );
        assert_eq!(diffs.len(), 1);

        let expected = json!(1);
        let actual = json!(1);
        let diffs = diff(&expected, &actual, Config::new(CompareMode::Inclusive));
        assert_eq!(diffs, vec![]);

        let expected = json!(2);
        let actual = json!(1);
        let diffs = diff(&expected, &actual, Config::new(CompareMode::Inclusive));
        assert_eq!(diffs.len(), 1);

        let expected = json!(1);
        let actual = json!(2);
        let diffs = diff(&expected, &actual, Config::new(CompareMode::Inclusive));
        assert_eq!(diffs.len(), 1);

        let expected = json!(1.0);
        let actual = json!(1.0);
        let diffs = diff(&expected, &actual, Config::new(CompareMode::Inclusive));
        assert_eq!(diffs, vec![]);

        let expected = json!(1);
        let actual = json!(1.0);
        let diffs = diff(&expected, &actual, Config::new(CompareMode::Inclusive));
        assert_eq!(diffs.len(), 1);

        let expected = json!(1.0);
        let actual = json!(1);
        let diffs = diff(&expected, &actual, Config::new(CompareMode::Inclusive));
        assert_eq!(diffs.len(), 1);

        let expected = json!(1);
        let actual = json!(1.0);
        let diffs = diff(
            &expected,
            &actual,
            Config::new(CompareMode::Inclusive).numeric_mode(NumericMode::AssumeFloat),
        );
        assert_eq!(diffs, vec![]);

        let expected = json!(1.0);
        let actual = json!(1);
        let diffs = diff(
            &expected,
            &actual,
            Config::new(CompareMode::Inclusive).numeric_mode(NumericMode::AssumeFloat),
        );
        assert_eq!(diffs, vec![]);
    }

    #[test]
    fn test_diffing_array() {
        // empty
        let expected = json!([]);
        let actual = json!([]);
        let diffs = diff(&expected, &actual, Config::new(CompareMode::Inclusive));
        assert_eq!(diffs, vec![]);

        let expected = json!([1]);
        let actual = json!([]);
        let diffs = diff(&expected, &actual, Config::new(CompareMode::Inclusive));
        assert_eq!(diffs.len(), 0);

        let expected = json!([]);
        let actual = json!([1]);
        let diffs = diff(&expected, &actual, Config::new(CompareMode::Inclusive));
        assert_eq!(diffs.len(), 1);

        // eq
        let expected = json!([1]);
        let actual = json!([1]);
        let diffs = diff(&expected, &actual, Config::new(CompareMode::Inclusive));
        assert_eq!(diffs, vec![]);

        // expected longer
        let expected = json!([1, 2]);
        let actual = json!([1]);
        let diffs = diff(&expected, &actual, Config::new(CompareMode::Inclusive));
        assert_eq!(diffs, vec![]);

        // actual longer
        let expected = json!([1]);
        let actual = json!([1, 2]);
        let diffs = diff(&expected, &actual, Config::new(CompareMode::Inclusive));
        assert_eq!(diffs.len(), 1);

        // eq length but different
        let expected = json!([1, 3]);
        let actual = json!([1, 2]);
        let diffs = diff(&expected, &actual, Config::new(CompareMode::Inclusive));
        assert_eq!(diffs.len(), 1);

        // different types
        let expected = json!(1);
        let actual = json!([1]);
        let diffs = diff(&expected, &actual, Config::new(CompareMode::Inclusive));
        assert_eq!(diffs.len(), 1);

        let expected = json!([1]);
        let actual = json!(1);
        let diffs = diff(&expected, &actual, Config::new(CompareMode::Inclusive));
        assert_eq!(diffs.len(), 1);
    }

    #[test]
    fn test_array_strict() {
        let expected = json!([]);
        let actual = json!([]);
        let diffs = diff(&expected, &actual, Config::new(CompareMode::Strict));
        assert_eq!(diffs.len(), 0);

        let expected = json!([1, 2]);
        let actual = json!([1, 2]);
        let diffs = diff(&expected, &actual, Config::new(CompareMode::Strict));
        assert_eq!(diffs.len(), 0);

        let expected = json!([1]);
        let actual = json!([1, 2]);
        let diffs = diff(&expected, &actual, Config::new(CompareMode::Strict));
        assert_eq!(diffs.len(), 1);

        let expected = json!([1, 2]);
        let actual = json!([1]);
        let diffs = diff(&expected, &actual, Config::new(CompareMode::Strict));
        assert_eq!(diffs.len(), 1);
    }

    #[test]
    fn test_object() {
        let expected = json!({});
        let actual = json!({});
        let diffs = diff(&expected, &actual, Config::new(CompareMode::Inclusive));
        assert_eq!(diffs, vec![]);

        let expected = json!({ "a": 1 });
        let actual = json!({ "a": 1 });
        let diffs = diff(&expected, &actual, Config::new(CompareMode::Inclusive));
        assert_eq!(diffs, vec![]);

        let expected = json!({ "a": 1, "b": 123 });
        let actual = json!({ "a": 1 });
        let diffs = diff(&expected, &actual, Config::new(CompareMode::Inclusive));
        assert_eq!(diffs, vec![]);

        let expected = json!({ "a": 1 });
        let actual = json!({ "b": 1 });
        let diffs = diff(&expected, &actual, Config::new(CompareMode::Inclusive));
        assert_eq!(diffs.len(), 1);

        let expected = json!({ "a": 1 });
        let actual = json!({ "a": 2 });
        let diffs = diff(&expected, &actual, Config::new(CompareMode::Inclusive));
        assert_eq!(diffs.len(), 1);

        let expected = json!({ "a": { "b": true } });
        let actual = json!({ "a": {} });
        let diffs = diff(&expected, &actual, Config::new(CompareMode::Inclusive));
        assert_eq!(diffs, vec![]);
    }

    #[test]
    fn test_object_strict() {
        let expected = json!({});
        let actual = json!({ "a": 1 });
        let diffs = diff(&expected, &actual, Config::new(CompareMode::Strict));
        assert_eq!(diffs.len(), 1);

        let expected = json!({ "a": 1 });
        let actual = json!({});
        let diffs = diff(&expected, &actual, Config::new(CompareMode::Strict));
        assert_eq!(diffs.len(), 1);

        let json = json!({ "a": 1 });
        let diffs = diff(&json, &json, Config::new(CompareMode::Strict));
        assert_eq!(diffs, vec![]);
    }

    #[test]
    fn test_object_deep_path() {
        let expected = json!({ "id": 1, "name": "John" });
        let actual = json!({ "id": 2, "name": "John" });
        let ignore_path = Path::from_jsonpath("$.id").unwrap();
        let diffs = diff(
            &expected,
            &actual,
            Config::new(CompareMode::Strict).ignore_path(ignore_path),
        );
        assert_eq!(diffs.len(), 0);

        let expected = json!({ "a": { "b": [{"c": 0}, { "c": 1 }] } });
        let actual = json!({ "a": { "b": [{"c": 0}, { "c": 2 }] } });
        let ignore_path = Path::from_jsonpath("$.a.b[*].c").unwrap();

        let config = Config::new(CompareMode::Strict).ignore_path(ignore_path);

        let diffs = diff(&expected, &actual, config);
        assert_eq!(diffs.len(), 0);

        // New test cases
        // Test deeper nesting with ignored path
        let expected = json!({ "a": { "b": { "d": { "e": 3 } } } });
        let actual = json!({ "a": { "b": { "d": { "e": 4 } } } });
        let ignore_path = Path::from_jsonpath("$.a.b.d.e").unwrap();

        let config = Config::new(CompareMode::Strict).ignore_path(ignore_path);
        let diffs = diff(&expected, &actual, config);
        assert_eq!(diffs.len(), 0);

        // Test array within deep object structure
        let expected = json!({ "a": { "b": [{ "d": [1, 2, 3] }] } });
        let actual = json!({ "a": { "b": [{ "d": [1, 2, 4] }] } });
        let ignore_path = Path::from_jsonpath("$.a.b[*].d[*]").unwrap();

        let config = Config::new(CompareMode::Strict).ignore_path(ignore_path);
        let diffs = diff(&expected, &actual, config);
        assert_eq!(diffs.len(), 0);

        // Test with multiple ignore paths
        let expected = json!({ "a": { "x": 1, "y": 2, "z": 3 } });
        let actual = json!({ "a": { "x": 1, "y": 3, "z": 3 } });
        let ignore_path1 = Path::from_jsonpath("$.a.x").unwrap();
        let ignore_path2 = Path::from_jsonpath("$.a.y").unwrap();

        let config = Config::new(CompareMode::Strict)
            .ignore_path(ignore_path1)
            .ignore_path(ignore_path2);
        let diffs = diff(&expected, &actual, config);
        assert_eq!(diffs.len(), 0);

        // Test ignored path with non-matching element
        let expected = json!({ "a": { "b": 1, "c": 2 } });
        let actual = json!({ "a": { "b": 1, "c": 3 } });
        let ignore_path = Path::from_jsonpath("$.a.d").unwrap();

        let config = Config::new(CompareMode::Strict).ignore_path(ignore_path);
        let diffs = diff(&expected, &actual, config);
        assert_ne!(diffs.len(), 0);

        let expected = json!({ "a": [ "b", "c" ] });
        let actual = json!({ "a": [ "c", "b" ] });
        let ignore_order_path = Path::from_jsonpath("$.a").unwrap();

        let config = Config::new(CompareMode::Strict).ignore_order(ignore_order_path);
        let diffs = diff(&expected, &actual, config);
        assert_eq!(diffs.len(), 0);

        let expected = json!({ "a": [ "b", "c" ] });
        let actual = json!({ "a": [ "c", "d" ] });
        let ignore_order_path = Path::from_jsonpath("$.a").unwrap();

        let config = Config::new(CompareMode::Strict).ignore_order(ignore_order_path);
        let diffs = diff(&expected, &actual, config);
        assert_eq!(diffs.len(), 2);
    }

    #[test]
    fn test_complex_jsons() {
        let expected_path = "tests/data/expected.json";
        let actual_path = "tests/data/actual.json";

        let expected_json =
            load_json_from_file(expected_path).expect("Error parsing expected.json");
        let actual_json = load_json_from_file(actual_path).expect("Error parsing actual.json");

        let diffs = diff(
            &expected_json,
            &actual_json,
            Config::new(CompareMode::Strict),
        );

        assert_eq!(diffs.len(), 26);

        let diffs = diff(
            &expected_json,
            &actual_json,
            Config::new(CompareMode::Strict).ignore_path("$.user.name".jsonpath().unwrap()),
        );

        assert_eq!(diffs.len(), 25);

        let diffs = diff(
            &expected_json,
            &actual_json,
            Config::new(CompareMode::Strict)
                .ignore_path("$.user.name".jsonpath().unwrap())
                .ignore_path("$.user.profile.age".jsonpath().unwrap()),
        );
        assert_eq!(diffs.len(), 24);

        let diffs = diff(
            &expected_json,
            &actual_json,
            Config::new(CompareMode::Strict)
                .ignore_path("$.user.name".jsonpath().unwrap())
                .ignore_path("$.user.profile.age".jsonpath().unwrap())
                .ignore_path("$.user.comments[*].timestamp".jsonpath().unwrap()),
        );
        assert_eq!(diffs.len(), 23);

        let diffs = diff(
            &expected_json,
            &actual_json,
            Config::new(CompareMode::Strict)
                .ignore_path("$.user.name".jsonpath().unwrap())
                .ignore_path("$.user.profile.age".jsonpath().unwrap())
                .ignore_path("$.user.comments[*].*".jsonpath().unwrap()),
        );
        for diff in &diffs {
            let path_str = format!("{}", diff.path);
            assert!(!path_str.starts_with(".user.comments"))
        }
        assert_eq!(diffs.len(), 20);

        let diffs = diff(
            &expected_json,
            &actual_json,
            Config::new(CompareMode::Strict)
                .ignore_path("$.user.name".jsonpath().unwrap())
                .ignore_path("$.user.profile.age".jsonpath().unwrap())
                .ignore_path("$.user.comments[*].*".jsonpath().unwrap())
                .ignore_order("$.system.components".jsonpath().unwrap()),
        );

        assert_eq!(diffs.len(), 14);
    }
}
