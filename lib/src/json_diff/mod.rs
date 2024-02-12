mod misc;
pub mod path;

use misc::{Indent, Indexes};
use path::{JSONPath, Key, Path};
use serde_json::Value;
use std::{collections::HashSet, fmt};

/// Mode for how JSON values should be compared.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub(crate) enum CompareMode {
    /// The two JSON values don't have to be exactly equal. The "actual" value is only required to
    /// be "contained" inside "expected". See [crate documentation](index.html) for examples.
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
pub(crate) struct Config<'a> {
    pub(crate) compare_mode: CompareMode,
    pub(crate) numeric_mode: NumericMode,
    pub(crate) ignore_paths: Vec<Path<'a>>,
}

impl<'a> Config<'a> {
    /// Create a new [`Config`] using the given [`CompareMode`].
    ///
    /// The default `numeric_mode` is be [`NumericMode::Strict`].
    pub fn new(compare_mode: CompareMode) -> Self {
        Self {
            compare_mode,
            numeric_mode: NumericMode::Strict,
            ignore_paths: vec![],
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
    pub fn ignore_path(mut self, path: Path<'a>) -> Self {
        self.ignore_paths.push(path);
        self
    }

    /// Checks if the given path should be ignored.
    pub fn to_ignore(&self, path: &Path<'a>) -> bool {
        self.ignore_paths.iter().any(|p| p.prefixes(path))
    }
}

pub(crate) fn diff<'a>(
    actual: &'a Value,
    expected: &'a Value,
    config: Config<'a>,
) -> Vec<Difference<'a>> {
    let mut acc = vec![];
    diff_with(actual, expected, config, Path::Root, &mut acc);
    acc
}

fn diff_with<'a>(
    actual: &'a Value,
    expected: &'a Value,
    config: Config<'a>,
    path: Path<'a>,
    acc: &mut Vec<Difference<'a>>,
) {
    let mut folder = DiffFolder {
        expected,
        path,
        acc,
        config,
    };

    fold_json(actual, &mut folder);
}

#[derive(Debug)]
struct DiffFolder<'a, 'b> {
    expected: &'a Value,
    path: Path<'a>,
    acc: &'b mut Vec<Difference<'a>>,
    config: Config<'a>,
}

macro_rules! direct_compare {
    ($name:ident) => {
        fn $name(&mut self, actual: &'a Value) {
            if self.expected != actual {
                if self.config.to_ignore(&self.path) {
                    return;
                }

                self.acc.push(Difference {
                    actual: Some(actual),
                    expected: Some(&self.expected),
                    path: self.path.clone(),
                    config: self.config.clone(),
                });
            }
        }
    };
}

impl<'a, 'b> DiffFolder<'a, 'b> {
    direct_compare!(on_null);
    direct_compare!(on_bool);
    direct_compare!(on_string);

    fn on_number(&mut self, actual: &'a Value) {
        let is_equal = match self.config.numeric_mode {
            NumericMode::Strict => self.expected == actual,
            NumericMode::AssumeFloat => self.expected.as_f64() == actual.as_f64(),
        };

        if self.config.to_ignore(&self.path) {
            return;
        }

        if !is_equal {
            self.acc.push(Difference {
                actual: Some(actual),
                expected: Some(self.expected),
                path: self.path.clone(),
                config: self.config.clone(),
            });
        }
    }

    fn on_array(&mut self, actual: &'a Value) {
        if let Some(expected) = self.expected.as_array() {
            let actual = actual.as_array().unwrap();

            match self.config.compare_mode {
                CompareMode::Inclusive => {
                    for (idx, expected) in expected.iter().enumerate() {
                        let path = self.path.append(Key::Idx(idx));

                        if let Some(actual) = actual.get(idx) {
                            diff_with(actual, expected, self.config.clone(), path, self.acc)
                        } else {
                            if self.config.to_ignore(&path) {
                                continue;
                            }

                            self.acc.push(Difference {
                                actual: None,
                                expected: Some(self.expected),
                                path,
                                config: self.config.clone(),
                            });
                        }
                    }
                }
                CompareMode::Strict => {
                    let all_keys = expected
                        .indexes()
                        .into_iter()
                        .chain(actual.indexes())
                        .collect::<HashSet<_>>();
                    for key in all_keys {
                        let path = self.path.append(Key::Idx(key));

                        match (actual.get(key), expected.get(key)) {
                            (Some(actual), Some(expected)) => {
                                diff_with(actual, expected, self.config.clone(), path, self.acc);
                            }
                            (None, Some(expected)) => {
                                if self.config.to_ignore(&path) {
                                    continue;
                                }

                                self.acc.push(Difference {
                                    actual: None,
                                    expected: Some(expected),
                                    path,
                                    config: self.config.clone(),
                                });
                            }
                            (Some(actual), None) => {
                                if self.config.to_ignore(&path) {
                                    continue;
                                }

                                self.acc.push(Difference {
                                    actual: Some(actual),
                                    expected: None,
                                    path,
                                    config: self.config.clone(),
                                });
                            }
                            (None, None) => {
                                unreachable!("at least one of the maps should have the key")
                            }
                        }
                    }
                }
            }
        } else {
            if self.config.to_ignore(&self.path) {
                return;
            }

            self.acc.push(Difference {
                actual: Some(actual),
                expected: Some(self.expected),
                path: self.path.clone(),
                config: self.config.clone(),
            });
        }
    }

    fn on_object(&mut self, actual: &'a Value) {
        if let Some(expected) = self.expected.as_object() {
            let actual = actual.as_object().unwrap();

            match self.config.compare_mode {
                CompareMode::Inclusive => {
                    for (key, expected) in expected.iter() {
                        let path = self.path.append(Key::Field(key));

                        if let Some(actual) = actual.get(key) {
                            diff_with(actual, expected, self.config.clone(), path, self.acc)
                        } else {
                            if self.config.to_ignore(&path) {
                                continue;
                            }

                            self.acc.push(Difference {
                                actual: None,
                                expected: Some(self.expected),
                                path,
                                config: self.config.clone(),
                            });
                        }
                    }
                }
                CompareMode::Strict => {
                    let all_keys = expected.keys().chain(actual.keys()).collect::<HashSet<_>>();
                    for key in all_keys {
                        let path = self.path.append(Key::Field(key));

                        match (actual.get(key), expected.get(key)) {
                            (Some(actual), Some(expected)) => {
                                diff_with(actual, expected, self.config.clone(), path, self.acc);
                            }
                            (None, Some(expected)) => {
                                if self.config.to_ignore(&path) {
                                    continue;
                                }

                                self.acc.push(Difference {
                                    actual: None,
                                    expected: Some(expected),
                                    path,
                                    config: self.config.clone(),
                                });
                            }
                            (Some(actual), None) => {
                                if self.config.to_ignore(&path) {
                                    continue;
                                }

                                self.acc.push(Difference {
                                    actual: Some(actual),
                                    expected: None,
                                    path,
                                    config: self.config.clone(),
                                });
                            }
                            (None, None) => {
                                unreachable!("at least one of the maps should have the key")
                            }
                        }
                    }
                }
            }
        } else {
            if self.config.to_ignore(&self.path) {
                return;
            }

            self.acc.push(Difference {
                actual: Some(actual),
                expected: Some(self.expected),
                path: self.path.clone(),
                config: self.config.clone(),
            });
        }
    }
}

#[derive(Debug, PartialEq)]
pub(crate) struct Difference<'a> {
    path: Path<'a>,
    actual: Option<&'a Value>,
    expected: Option<&'a Value>,
    config: Config<'a>,
}

impl<'a> fmt::Display for Difference<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let json_to_string = |json: &Value| serde_json::to_string_pretty(json).unwrap();

        match (&self.config.compare_mode, &self.actual, &self.expected) {
            (CompareMode::Inclusive, Some(actual), Some(expected)) => {
                writeln!(f, "json atoms at path \"{}\" are not equal:", self.path)?;
                writeln!(f, "    expected:")?;
                writeln!(f, "{}", json_to_string(expected).indent(8))?;
                writeln!(f, "    actual:")?;
                write!(f, "{}", json_to_string(actual).indent(8))?;
            }
            (CompareMode::Inclusive, None, Some(_expected)) => {
                write!(
                    f,
                    "json atom at path \"{}\" is missing from actual",
                    self.path
                )?;
            }
            (CompareMode::Inclusive, Some(_actual), None) => {
                unreachable!("stuff missing actual wont produce an error")
            }
            (CompareMode::Inclusive, None, None) => unreachable!("can't both be missing"),

            (CompareMode::Strict, Some(actual), Some(expected)) => {
                writeln!(f, "json atoms at path \"{}\" are not equal:", self.path)?;
                writeln!(f, "    expected:")?;
                writeln!(f, "{}", json_to_string(actual).indent(8))?;
                writeln!(f, "    actual:")?;
                write!(f, "{}", json_to_string(expected).indent(8))?;
            }
            (CompareMode::Strict, None, Some(_)) => {
                write!(
                    f,
                    "json atom at path \"{}\" is missing from actual",
                    self.path
                )?;
            }
            (CompareMode::Strict, Some(_), None) => {
                write!(
                    f,
                    "json atom at path \"{}\" is missing from expected",
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
        Value::Array(_) => folder.on_array(json),
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

        let actual = json!(1);
        let expected = json!(1);
        let diffs = diff(&actual, &expected, Config::new(CompareMode::Inclusive));
        assert_eq!(diffs, vec![]);

        let actual = json!(2);
        let expected = json!(1);
        let diffs = diff(&actual, &expected, Config::new(CompareMode::Inclusive));
        assert_eq!(diffs.len(), 1);

        let actual = json!(1);
        let expected = json!(2);
        let diffs = diff(&actual, &expected, Config::new(CompareMode::Inclusive));
        assert_eq!(diffs.len(), 1);

        let actual = json!(1.0);
        let expected = json!(1.0);
        let diffs = diff(&actual, &expected, Config::new(CompareMode::Inclusive));
        assert_eq!(diffs, vec![]);

        let actual = json!(1);
        let expected = json!(1.0);
        let diffs = diff(&actual, &expected, Config::new(CompareMode::Inclusive));
        assert_eq!(diffs.len(), 1);

        let actual = json!(1.0);
        let expected = json!(1);
        let diffs = diff(&actual, &expected, Config::new(CompareMode::Inclusive));
        assert_eq!(diffs.len(), 1);

        let actual = json!(1);
        let expected = json!(1.0);
        let diffs = diff(
            &actual,
            &expected,
            Config::new(CompareMode::Inclusive).numeric_mode(NumericMode::AssumeFloat),
        );
        assert_eq!(diffs, vec![]);

        let actual = json!(1.0);
        let expected = json!(1);
        let diffs = diff(
            &actual,
            &expected,
            Config::new(CompareMode::Inclusive).numeric_mode(NumericMode::AssumeFloat),
        );
        assert_eq!(diffs, vec![]);
    }

    #[test]
    fn test_diffing_array() {
        // empty
        let actual = json!([]);
        let expected = json!([]);
        let diffs = diff(&actual, &expected, Config::new(CompareMode::Inclusive));
        assert_eq!(diffs, vec![]);

        let actual = json!([1]);
        let expected = json!([]);
        let diffs = diff(&actual, &expected, Config::new(CompareMode::Inclusive));
        assert_eq!(diffs.len(), 0);

        let actual = json!([]);
        let expected = json!([1]);
        let diffs = diff(&actual, &expected, Config::new(CompareMode::Inclusive));
        assert_eq!(diffs.len(), 1);

        // eq
        let actual = json!([1]);
        let expected = json!([1]);
        let diffs = diff(&actual, &expected, Config::new(CompareMode::Inclusive));
        assert_eq!(diffs, vec![]);

        // actual longer
        let actual = json!([1, 2]);
        let expected = json!([1]);
        let diffs = diff(&actual, &expected, Config::new(CompareMode::Inclusive));
        assert_eq!(diffs, vec![]);

        // expected longer
        let actual = json!([1]);
        let expected = json!([1, 2]);
        let diffs = diff(&actual, &expected, Config::new(CompareMode::Inclusive));
        assert_eq!(diffs.len(), 1);

        // eq length but different
        let actual = json!([1, 3]);
        let expected = json!([1, 2]);
        let diffs = diff(&actual, &expected, Config::new(CompareMode::Inclusive));
        assert_eq!(diffs.len(), 1);

        // different types
        let actual = json!(1);
        let expected = json!([1]);
        let diffs = diff(&actual, &expected, Config::new(CompareMode::Inclusive));
        assert_eq!(diffs.len(), 1);

        let actual = json!([1]);
        let expected = json!(1);
        let diffs = diff(&actual, &expected, Config::new(CompareMode::Inclusive));
        assert_eq!(diffs.len(), 1);
    }

    #[test]
    fn test_array_strict() {
        let actual = json!([]);
        let expected = json!([]);
        let diffs = diff(&actual, &expected, Config::new(CompareMode::Strict));
        assert_eq!(diffs.len(), 0);

        let actual = json!([1, 2]);
        let expected = json!([1, 2]);
        let diffs = diff(&actual, &expected, Config::new(CompareMode::Strict));
        assert_eq!(diffs.len(), 0);

        let actual = json!([1]);
        let expected = json!([1, 2]);
        let diffs = diff(&actual, &expected, Config::new(CompareMode::Strict));
        assert_eq!(diffs.len(), 1);

        let actual = json!([1, 2]);
        let expected = json!([1]);
        let diffs = diff(&actual, &expected, Config::new(CompareMode::Strict));
        assert_eq!(diffs.len(), 1);
    }

    #[test]
    fn test_object() {
        let actual = json!({});
        let expected = json!({});
        let diffs = diff(&actual, &expected, Config::new(CompareMode::Inclusive));
        assert_eq!(diffs, vec![]);

        let actual = json!({ "a": 1 });
        let expected = json!({ "a": 1 });
        let diffs = diff(&actual, &expected, Config::new(CompareMode::Inclusive));
        assert_eq!(diffs, vec![]);

        let actual = json!({ "a": 1, "b": 123 });
        let expected = json!({ "a": 1 });
        let diffs = diff(&actual, &expected, Config::new(CompareMode::Inclusive));
        assert_eq!(diffs, vec![]);

        let actual = json!({ "a": 1 });
        let expected = json!({ "b": 1 });
        let diffs = diff(&actual, &expected, Config::new(CompareMode::Inclusive));
        assert_eq!(diffs.len(), 1);

        let actual = json!({ "a": 1 });
        let expected = json!({ "a": 2 });
        let diffs = diff(&actual, &expected, Config::new(CompareMode::Inclusive));
        assert_eq!(diffs.len(), 1);

        let actual = json!({ "a": { "b": true } });
        let expected = json!({ "a": {} });
        let diffs = diff(&actual, &expected, Config::new(CompareMode::Inclusive));
        assert_eq!(diffs, vec![]);
    }

    #[test]
    fn test_object_strict() {
        let actual = json!({});
        let expected = json!({ "a": 1 });
        let diffs = diff(&actual, &expected, Config::new(CompareMode::Strict));
        assert_eq!(diffs.len(), 1);

        let actual = json!({ "a": 1 });
        let expected = json!({});
        let diffs = diff(&actual, &expected, Config::new(CompareMode::Strict));
        assert_eq!(diffs.len(), 1);

        let json = json!({ "a": 1 });
        let diffs = diff(&json, &json, Config::new(CompareMode::Strict));
        assert_eq!(diffs, vec![]);
    }

    #[test]
    fn test_object_deep_path() {
        let actual = json!({ "id": 1, "name": "John" });
        let expected = json!({ "id": 2, "name": "John" });
        let ignore_path = Path::from_jsonpath("$.id").unwrap();
        let diffs = diff(
            &actual,
            &expected,
            Config::new(CompareMode::Strict).ignore_path(ignore_path),
        );
        assert_eq!(diffs.len(), 0);

        let actual = json!({ "a": { "b": [{"c": 0}, { "c": 1 }] } });
        let expected = json!({ "a": { "b": [{"c": 0}, { "c": 2 }] } });
        let ignore_path = Path::from_jsonpath("$.a.b[*].c").unwrap();

        let config = Config::new(CompareMode::Strict).ignore_path(ignore_path);

        let diffs = diff(&actual, &expected, config);
        assert_eq!(diffs.len(), 0);

        // New test cases
        // Test deeper nesting with ignored path
        let actual = json!({ "a": { "b": { "d": { "e": 3 } } } });
        let expected = json!({ "a": { "b": { "d": { "e": 4 } } } });
        let ignore_path = Path::from_jsonpath("$.a.b.d.e").unwrap();

        let config = Config::new(CompareMode::Strict).ignore_path(ignore_path);
        let diffs = diff(&actual, &expected, config);
        assert_eq!(diffs.len(), 0);

        // Test array within deep object structure
        let actual = json!({ "a": { "b": [{ "d": [1, 2, 3] }] } });
        let expected = json!({ "a": { "b": [{ "d": [1, 2, 4] }] } });
        let ignore_path = Path::from_jsonpath("$.a.b[*].d[*]").unwrap();

        let config = Config::new(CompareMode::Strict).ignore_path(ignore_path);
        let diffs = diff(&actual, &expected, config);
        assert_eq!(diffs.len(), 0);

        // Test with multiple ignore paths
        let actual = json!({ "a": { "x": 1, "y": 2, "z": 3 } });
        let expected = json!({ "a": { "x": 1, "y": 3, "z": 3 } });
        let ignore_path1 = Path::from_jsonpath("$.a.x").unwrap();
        let ignore_path2 = Path::from_jsonpath("$.a.y").unwrap();

        let config = Config::new(CompareMode::Strict)
            .ignore_path(ignore_path1)
            .ignore_path(ignore_path2);
        let diffs = diff(&actual, &expected, config);
        assert_eq!(diffs.len(), 0);

        // Test ignored path with non-matching element
        let actual = json!({ "a": { "b": 1, "c": 2 } });
        let expected = json!({ "a": { "b": 1, "c": 3 } });
        let ignore_path = Path::from_jsonpath("$.a.d").unwrap();

        let config = Config::new(CompareMode::Strict).ignore_path(ignore_path);
        let diffs = diff(&actual, &expected, config);
        assert_ne!(diffs.len(), 0);
    }

    #[test]
    fn test_complex_jsons() {
        let actual_path = "tests/data/actual.json";
        let expected_path = "tests/data/expected.json";

        let actual_json = load_json_from_file(actual_path).expect("Error parsing actual.json");
        let expected_json =
            load_json_from_file(expected_path).expect("Error parsing expected.json");

        let diffs = diff(
            &actual_json,
            &expected_json,
            Config::new(CompareMode::Inclusive),
        );
        assert_eq!(diffs.len(), 20);

        let diffs = diff(
            &actual_json,
            &expected_json,
            Config::new(CompareMode::Strict).ignore_path("$.user.name".jsonpath().unwrap()),
        );
        assert_eq!(diffs.len(), 19);

        let diffs = diff(
            &actual_json,
            &expected_json,
            Config::new(CompareMode::Strict)
                .ignore_path("$.user.name".jsonpath().unwrap())
                .ignore_path("$.user.profile.age".jsonpath().unwrap()),
        );
        assert_eq!(diffs.len(), 18);

        let diffs = diff(
            &actual_json,
            &expected_json,
            Config::new(CompareMode::Strict)
                .ignore_path("$.user.name".jsonpath().unwrap())
                .ignore_path("$.user.profile.age".jsonpath().unwrap())
                .ignore_path("$.user.comments[*].timestamp".jsonpath().unwrap()),
        );
        assert_eq!(diffs.len(), 17);

        let diffs = diff(
            &actual_json,
            &expected_json,
            Config::new(CompareMode::Strict)
                .ignore_path("$.user.name".jsonpath().unwrap())
                .ignore_path("$.user.profile.age".jsonpath().unwrap())
                .ignore_path("$.user.comments[*].*".jsonpath().unwrap()),
        );
        for diff in &diffs {
            let path_str = format!("{}", diff.path);
            assert!(!path_str.starts_with(".user.comments"))
        }
        assert_eq!(diffs.len(), 14);
    }
}
