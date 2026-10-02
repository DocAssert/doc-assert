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
use serde_json::Value;
use std::collections::HashMap;

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

    pub(crate) fn obtain_from_response(
        &mut self,
        response: &Value,
        variable_templates: &HashMap<String, Path>,
    ) -> Result<(), Mismatch> {
        for (name, path) in variable_templates {
            let value = extract_value(path, response)
                .ok_or_else(|| Mismatch::VariableNotFound { name: name.clone() })?;

            self.map.insert(name.clone(), value);
        }

        Ok(())
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

        if input.contains('`') {
            return Err(Failure::UnresolvedVariables {
                input: input.clone(),
            });
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
