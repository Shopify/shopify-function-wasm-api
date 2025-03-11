use shopify_function_wasm_api_core::{NanBox, ValueRef};

#[link(wasm_import_module = "shopify_function_v0.1.0")]
extern "C" {
    // Read API.
    fn shopify_function_input_get() -> u64;
    fn shopify_function_input_read_utf8_str(src: usize, out: *mut u8, len: usize);
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

    pub fn as_number(&self) -> Option<f64> {
        match self {
            Value::NanBox(v) => match v.try_decode() {
                Ok(ValueRef::Number(n)) => Some(n),
                _ => None,
            },
        }
    }

    pub fn as_string(&self) -> Option<String> {
        match self {
            Value::NanBox(v) => match v.try_decode() {
                Ok(ValueRef::String { ptr, len }) => {
                    let mut buf = vec![0; len];
                    unsafe {
                        shopify_function_input_read_utf8_str(ptr as _, buf.as_mut_ptr(), len)
                    };
                    Some(unsafe { String::from_utf8_unchecked(buf) })
                }
                _ => None,
            },
        }
    }
}

pub fn input_get() -> Value {
    let val = unsafe { shopify_function_input_get() };
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
