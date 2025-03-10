use shopify_function_wasm_api_core::{NanBox, ValueRef};

#[link(wasm_import_module = "trampoline")]
extern "C" {
    // Read API.
    fn trampoline_input_get() -> u64;
}

pub enum Value {
    NanBox(NanBox),
}

impl Value {
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Value::NanBox(v) => match v.try_decode() {
                Ok(ValueRef::Bool(b)) => Some(b),
                _ => None,
            },
        }
    }

    pub fn as_null(&self) -> Option<()> {
        match self {
            Value::NanBox(v) => match v.try_decode() {
                Ok(ValueRef::Null) => Some(()),
                _ => None,
            },
        }
    }
}

pub fn input_get() -> Value {
    let val = unsafe { trampoline_input_get() };
    Value::NanBox(NanBox::from_bits(val))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bool_roundtrip() {
        [true, false].iter().for_each(|&val| {
            let boxed = Value::NanBox(NanBox::bool(val));
            assert_eq!(boxed.as_bool(), Some(val));
        });
    }

    #[test]
    fn test_null_roundtrip() {
        let boxed = Value::NanBox(NanBox::null());
        assert_eq!(boxed.as_null(), Some(()));
    }
}
