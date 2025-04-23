use serde_json::Value as JsonValue;

use crate::InternedStringId;

#[derive(Clone)]
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
        self.value.as_bool()
    }

    pub fn is_null(&self) -> bool {
        self.value.is_null()
    }

    pub fn as_number(&self) -> Option<f64> {
        self.value.as_number().map(|n| n.as_f64().unwrap())
    }

    pub fn as_string(&self) -> Option<String> {
        self.value.as_str().map(|s| s.to_string())
    }

    pub fn is_obj(&self) -> bool {
        self.value.is_object()
    }

    pub fn get_obj_prop(&self, prop: &str) -> Self {
        self.value
            .get(prop)
            .map(|value| Self {
                value: value.clone(),
                interned_strings: self.interned_strings.clone(),
            })
            .unwrap_or_else(|| Self {
                value: JsonValue::Null,
                interned_strings: self.interned_strings.clone(),
            })
    }

    pub fn get_interned_obj_prop(&self, interned_string_id: InternedStringId) -> Self {
        let interned_strings = &self.interned_strings;
        let prop = &interned_strings[interned_string_id.as_usize()];

        self.value
            .get(prop)
            .map(|value| Self {
                value: value.clone(),
                interned_strings: self.interned_strings.clone(),
            })
            .unwrap_or_else(|| Self {
                value: JsonValue::Null,
                interned_strings: self.interned_strings.clone(),
            })
    }

    pub fn is_array(&self) -> bool {
        self.value.is_array()
    }

    pub fn array_len(&self) -> Option<usize> {
        self.value.as_array().map(|arr| arr.len())
    }

    pub fn get_at_index(&self, index: usize) -> Value {
        self.value.as_array().and_then(|arr| arr.get(index)).map_or(
            Self {
                value: JsonValue::Null,
                interned_strings: self.interned_strings.clone(),
            },
            |value| Self {
                value: value.clone(),
                interned_strings: self.interned_strings.clone(),
            },
        )
    }
}