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

//! Values shared between test cases, and the substitution of their placeholders.

use crate::{
    domain::{Request, Response},
    json_diff::path::{Key, Path},
    report::{Failure, Mismatch},
};
use regex::Regex;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::OnceLock;

/// Variables to be used in the request and response bodies.
///
/// The variables replace placeholders in the request and response bodies, in case some
/// values need to be shared between requests.
///
/// # Examples
///
/// Variables can be inserted one by one:
///
/// ```
/// # use doc_assert::Variables;
/// let mut variables = Variables::new();
/// variables.insert("name", "John");
/// variables.insert("age", 30);
/// ```
///
/// Alternatively, they can be passed as a JSON object:
///
/// ```
/// # use doc_assert::Variables;
/// let json = r#"{"name": "John", "age": 30}"#;
/// let variables = Variables::from_json(&serde_json::from_str(json).unwrap()).unwrap();
/// ```
#[derive(Debug, Clone, Default)]
pub struct Variables {
    map: HashMap<String, Value>,
}

impl Variables {
    /// Constructs a new, empty `Variables`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use doc_assert::Variables;
    /// let variables = Variables::new();
    /// ```
    pub fn new() -> Self {
        Self {
            map: HashMap::new(),
        }
    }

    /// Constructs a new `Variables` from a JSON object, `None` if `json` is not an object.
    ///
    /// # Examples
    ///
    /// ```
    /// # use doc_assert::Variables;
    /// let json = r#"{"name": "John", "age": 30}"#;
    /// let variables = Variables::from_json(&serde_json::from_str(json).unwrap()).unwrap();
    /// ```
    pub fn from_json(json: &Value) -> Option<Self> {
        let obj = json.as_object()?;
        Some(Self {
            map: obj.clone().into_iter().collect(),
        })
    }

    /// Inserts a variable, overwriting any variable of the same name.
    ///
    /// Anything a `serde_json::Value` can be built from is accepted, which covers the
    /// strings, numbers and booleans a documentation usually needs. Pass a `Value`
    /// itself for the types it does not cover.
    ///
    /// # Examples
    ///
    /// ```
    /// # use doc_assert::Variables;
    /// # use serde_json::Value;
    /// let mut variables = Variables::new();
    /// variables.insert("name", "John");
    /// variables.insert("age", 30);
    /// variables.insert("is_adult", true);
    /// variables.insert("nickname", Value::Null);
    /// ```
    pub fn insert(&mut self, name: impl Into<String>, value: impl Into<Value>) {
        self.map.insert(name.into(), value.into());
    }

    /// Extracts the variables the documentation defines from a response body, leaving the
    /// ones already known untouched: they are only updated once the response passed, see
    /// [`Variables::extend`].
    pub(crate) fn extract_from_response(
        response: &Value,
        variable_templates: &HashMap<String, Path>,
    ) -> Result<HashMap<String, Value>, Mismatch> {
        variable_templates
            .iter()
            .map(|(name, path)| {
                extract_value(path, response)
                    .map(|value| (name.clone(), value))
                    .ok_or_else(|| Mismatch::VariableNotFound { name: name.clone() })
            })
            .collect()
    }

    pub(crate) fn extend(&mut self, variables: HashMap<String, Value>) {
        self.map.extend(variables);
    }

    fn replace_placeholders(&self, input: &mut String, trim_quotes: bool) -> Result<(), Failure> {
        for (name, value) in &self.map {
            let placeholder = format!("`{}`", name);
            let value_str = value.to_string();

            let value = if trim_quotes {
                value_str.trim_matches('"')
            } else {
                value_str.as_str()
            };

            *input = input.replace(&placeholder, value);
        }

        // only the names are reported: the input may hold the values of other variables,
        // such as tokens, which should not end up in a report
        let names = unresolved_placeholders(input);
        if !names.is_empty() {
            return Err(Failure::UnresolvedVariables { names });
        }

        Ok(())
    }

    pub(crate) fn replace_request_placeholders(&self, input: &mut Request) -> Result<(), Failure> {
        self.replace_placeholders(&mut input.uri, true)?;

        if let Some(body) = &mut input.body {
            self.replace_placeholders(body, false)?;
        }

        for (_, value) in &mut input.headers.iter_mut() {
            self.replace_placeholders(value, true)?;
        }

        Ok(())
    }

    pub(crate) fn replace_response_placeholders(
        &self,
        input: &mut Response,
    ) -> Result<(), Failure> {
        if let Some(body) = &mut input.body {
            self.replace_placeholders(body, false)?;
        }

        for (_, value) in &mut input.headers.iter_mut() {
            self.replace_placeholders(value, true)?;
        }

        Ok(())
    }
}

/// Names of the placeholders left in `input`, in the order they appear and without
/// repetitions.
fn unresolved_placeholders(input: &str) -> Vec<String> {
    static PLACEHOLDER: OnceLock<Regex> = OnceLock::new();
    let placeholder = PLACEHOLDER.get_or_init(|| Regex::new(r"`([^`\s]+)`").unwrap());

    let mut names: Vec<String> = vec![];
    for caps in placeholder.captures_iter(input) {
        let name = &caps[1];
        if !names.iter().any(|n| n == name) {
            names.push(name.to_string());
        }
    }
    names
}

fn extract_value(path: &Path, value: &Value) -> Option<Value> {
    match path {
        Path::Root => None,
        Path::Keys(keys) => {
            let mut current = value;
            for key in keys {
                match key {
                    Key::Field(field) => current = current.get(field)?,
                    Key::Idx(index) => current = current.get(index)?,
                    _ => return None,
                }
            }
            Some(current.clone())
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use serde_json::{json, Value};

    use super::Variables;
    use crate::json_diff::path::JSONPath;
    use crate::report::{Failure, Mismatch};

    #[test]
    fn test_from_json_accepts_only_an_object() {
        assert!(Variables::from_json(&json!({"id": 1})).is_some());
        for json in [json!("id"), json!([1]), json!(1), Value::Null] {
            assert!(Variables::from_json(&json).is_none(), "{}", json);
        }
    }

    #[test]
    fn test_insert_overwrites_a_variable_of_the_same_name() {
        let mut variables = Variables::new();
        variables.insert("id", "x");
        variables.insert("id", 1);

        let mut input = "/users/`id`".to_string();
        variables.replace_placeholders(&mut input, true).unwrap();
        assert_eq!("/users/1", input);
    }

    #[test]
    fn test_unresolved_placeholders_are_reported_by_name_only() {
        let mut variables = Variables::new();
        variables.insert("token", "secret");

        let mut input = "`token` `id` `name` `id`".to_string();
        let err = variables
            .replace_placeholders(&mut input, false)
            .unwrap_err();

        assert_eq!(
            Failure::UnresolvedVariables {
                names: vec!["id".to_string(), "name".to_string()]
            },
            err
        );
        assert!(!err.to_string().contains("secret"));
    }

    #[test]
    fn test_a_lone_backtick_is_not_a_placeholder() {
        let mut input = "{\"quote\": \"it`s\"}".to_string();
        assert_eq!(
            Ok(()),
            Variables::new().replace_placeholders(&mut input, false)
        );
    }

    #[test]
    fn test_every_variable_or_none_is_extracted_from_a_response() {
        let templates: HashMap<_, _> = [
            ("id".to_string(), "$.id".jsonpath().unwrap()),
            ("name".to_string(), "$.name".jsonpath().unwrap()),
        ]
        .into_iter()
        .collect();

        let extracted =
            Variables::extract_from_response(&json!({"id": 1, "name": "John"}), &templates)
                .unwrap();
        assert_eq!(Some(&json!(1)), extracted.get("id"));
        assert_eq!(Some(&json!("John")), extracted.get("name"));

        assert_eq!(
            Err(Mismatch::VariableNotFound {
                name: "name".to_string()
            }),
            Variables::extract_from_response(&json!({"id": 1}), &templates)
        );
    }
}
