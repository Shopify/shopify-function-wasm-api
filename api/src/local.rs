use serde_json::Value as JsonValue;
use shopify_function_wasm_api_core::ContextPtr;

use crate::InternedStringId;

pub struct Value {
    value: JsonValue,
    interned_strings: Vec<String>,
}

impl Value {
    pub fn new(value: JsonValue, interned_strings: Vec<String>) -> Self {
        Self {
            value,
            interned_strings,
        }
    }

    pub fn intern_utf8_str(&mut self, s: &str) -> InternedStringId {
        self.interned_strings.push(s.to_string());
        InternedStringId(self.interned_strings.len() - 1)
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self.value {
            JsonValue::Bool(b) => Some(b),
            _ => None,
        }
    }

    pub fn is_null(&self) -> bool {
        matches!(self.value, JsonValue::Null)
    }

    pub fn as_number(&self) -> Option<f64> {
        match &self.value {
            JsonValue::Number(n) => Some(n.as_f64().unwrap()),
            _ => None,
        }
    }

    pub fn as_string(&self) -> Option<String> {
        match &self.value {
            JsonValue::String(s) => Some(s.clone()),
            _ => None,
        }
    }

    pub fn is_obj(&self) -> bool {
        matches!(self.value, JsonValue::Object { .. })
    }

    pub fn get_obj_prop(&self, prop: &str) -> Self {
        match &self.value {
            JsonValue::Object(obj) => {
                let value = obj.get(prop).unwrap();
                Self {
                    value: value.clone(),
                    interned_strings: self.interned_strings.clone(),
                }
            }
            _ => Self {
                value: JsonValue::Null,
                interned_strings: self.interned_strings.clone(),
            },
        }
    }

    pub fn get_interned_obj_prop(&self, interned_string_id: InternedStringId) -> Self {
        match &self.value {
            JsonValue::Object(obj) => {
                let interned_strings = &self.interned_strings;
                let prop = &interned_strings[interned_string_id.as_usize()];
                let value = obj.get(prop).unwrap();
                Self {
                    value: value.clone(),
                    interned_strings: self.interned_strings.clone(),
                }
            }
            _ => Self {
                value: JsonValue::Null,
                interned_strings: self.interned_strings.clone(),
            },
        }
    }

    pub fn is_array(&self) -> bool {
        matches!(self.value, JsonValue::Array { .. })
    }

    pub fn array_len(&self) -> Option<usize> {
        match &self.value {
            JsonValue::Array(arr) => Some(arr.len()),
            _ => None,
        }
    }

    pub fn get_at_index(&self, index: usize) -> Value {
        match &self.value {
            JsonValue::Array(arr) => {
                let value = &arr[index];
                Self {
                    value: value.clone(),
                    interned_strings: self.interned_strings.clone(),
                }
            }
            _ => Self {
                value: JsonValue::Null,
                interned_strings: self.interned_strings.clone(),
            },
        }
    }
}

pub struct Context(pub ContextPtr);

impl Context {
    pub fn new() -> Self {
        Self(std::ptr::null_mut())
    }
}

impl Default for Context {
    fn default() -> Self {
        Self::new()
    }
}
