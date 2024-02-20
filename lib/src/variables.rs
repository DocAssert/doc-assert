use std::collections::HashMap;

use serde_json::Value;

use crate::{
    domain::{Request, Response},
    json_diff::path::{Key, Path},
};

#[derive(Debug, Clone)]
pub struct Variables {
    pub map: HashMap<String, Value>,
}

impl Variables {
    pub fn new() -> Self {
        Self {
            map: HashMap::new(),
        }
    }

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

impl Default for Variables {
    fn default() -> Self {
        Self::new()
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
