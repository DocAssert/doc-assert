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

use std::collections::HashMap;

use serde_json::Value;

use crate::{
    domain::{Request, Response},
    json_diff::path::{Key, Path},
};

/// Variables to be used in the request and response bodies.
///
/// The variables are used to replace placeholders in the request
/// and response bodies in case some values need to be shared between requests and responses.
///
/// # Examples
///
/// Variables can be passed one by one with specified type:
///
/// ```
/// # use doc_assert::variables::Variables;
/// # use serde_json::Value;
/// let mut variables = Variables::new();
/// variables.insert_string("name".to_string(), "John".to_string());
/// variables.insert_int("age".to_string(), 30);
/// ```
///
/// A `Value` can be passed directly:
///
/// ```
/// # use doc_assert::variables::Variables;
/// # use serde_json::Value;
/// let mut variables = Variables::new();
/// variables.insert_value("name".to_string(), Value::String("John".to_string()));
/// variables.insert_value("age".to_string(), Value::Number(serde_json::Number::from(30)));
/// ```
///
/// Alternatively, they can be passed as a JSON object:
///
/// ```
/// # use doc_assert::variables::Variables;
/// # use serde_json::Value;
/// let json = r#"{"name": "John", "age": 30}"#;
/// let variables = Variables::from_json(&serde_json::from_str(json).unwrap()).unwrap();
/// ```
///
#[derive(Debug, Clone, Default)]
pub struct Variables {
    map: HashMap<String, Value>,
}

impl Variables {
    /// Constructs a new `Variables`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use doc_assert::variables::Variables;
    /// let variables = Variables::new();
    /// ```
    pub fn new() -> Self {
        Self {
            map: HashMap::new(),
        }
    }

    /// Constructs a new `Variables` from a JSON object.
    ///
    /// # Examples
    ///
    /// ```
    /// # use doc_assert::variables::Variables;
    /// # use serde_json::Value;
    /// let json = r#"{"name": "John", "age": 30}"#;
    /// let variables = Variables::from_json(&serde_json::from_str(json).unwrap()).unwrap();
    /// ```
    pub fn from_json(json: &Value) -> Result<Self, String> {
        let mut map = HashMap::new();

        if let Value::Object(obj) = json {
            for (key, value) in obj {
                map.insert(key.clone(), value.clone());
            }
        } else {
            return Err("variables must be an object".to_string());
        }

        Ok(Self { map })
    }

    /// Inserts a `Value` into the `Variables`.
    ///
    /// This can be useful when more complex types are needed.
    ///
    /// # Examples
    ///
    /// ```
    /// # use doc_assert::variables::Variables;
    /// # use serde_json::Value;
    /// let mut variables = Variables::new();
    /// variables.insert_value("name".to_string(), Value::String("John".to_string()));
    /// ```
    pub fn insert_value(&mut self, name: String, value: Value) {
        self.map.insert(name, value);
    }

    /// Inserts a `String` into the `Variables`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use doc_assert::variables::Variables;
    /// let mut variables = Variables::new();
    /// variables.insert_string("name".to_string(), "John".to_string());
    /// ```
    pub fn insert_string(&mut self, name: String, value: String) {
        self.map.insert(name, Value::String(value));
    }

    /// Inserts an `i64` into the `Variables`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use doc_assert::variables::Variables;
    /// let mut variables = Variables::new();
    /// variables.insert_int("age".to_string(), 30);
    /// ```
    pub fn insert_int(&mut self, name: String, value: i64) {
        self.map
            .insert(name, Value::Number(serde_json::Number::from(value)));
    }

    /// Inserts an `f64` into the `Variables`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use doc_assert::variables::Variables;
    /// let mut variables = Variables::new();
    /// variables.insert_float("age".to_string(), 30.0);
    /// ```
    pub fn insert_float(&mut self, name: String, value: f64) {
        self.map.insert(
            name,
            Value::Number(serde_json::Number::from_f64(value).unwrap()),
        );
    }

    /// Inserts a `bool` into the `Variables`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use doc_assert::variables::Variables;
    /// let mut variables = Variables::new();
    /// variables.insert_bool("is_adult".to_string(), true);
    /// ```
    pub fn insert_bool(&mut self, name: String, value: bool) {
        self.map.insert(name, Value::Bool(value));
    }

    /// Inserts a `null` into the `Variables`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use doc_assert::variables::Variables;
    /// let mut variables = Variables::new();
    /// variables.insert_null("name".to_string());
    /// ```
    pub fn insert_null(&mut self, name: String) {
        self.map.insert(name, Value::Null);
    }

    pub(crate) fn obtain_from_response(
        &mut self,
        response: &Value,
        variable_templates: &HashMap<String, Path>,
    ) -> Result<(), String> {
        for (name, path) in variable_templates {
            let value = extract_value(path, response).ok_or_else(|| {
                format!("variable template {} not found in the response body", name)
            })?;

            self.map.insert(name.clone(), value);
        }

        Ok(())
    }

    fn replace_placeholders(&self, input: &mut String, trim_quotes: bool) -> Result<(), String> {
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
            return Err(format!("unresolved variable placeholders in {}", input));
        }

        Ok(())
    }

    pub(crate) fn replace_request_placeholders(&self, input: &mut Request) -> Result<(), String> {
        self.replace_placeholders(&mut input.uri, true)?;

        if let Some(body) = &mut input.body {
            self.replace_placeholders(body, false)?;
        }

        for (_, value) in &mut input.headers.iter_mut() {
            self.replace_placeholders(value, true)?;
        }

        Ok(())
    }

    pub(crate) fn replace_response_placeholders(&self, input: &mut Response) -> Result<(), String> {
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
