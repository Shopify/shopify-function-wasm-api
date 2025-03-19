use shopify_function_wasm_api_core::{NanBox, ValueRef};

#[link(wasm_import_module = "shopify_function_v0.1.0")]
extern "C" {
    // Read API.
    fn shopify_function_input_get() -> u64;
    fn shopify_function_input_get_length(ptr: usize) -> u64; // does this need to be in the trampoline as well?
    fn shopify_function_input_read_utf8_str(src: usize, out: *mut u8, len: usize);
    fn shopify_function_input_get_obj_prop(scope: u64, ptr: *const u8, len: usize) -> u64;
    fn shopify_function_input_get_at_index(scope: u64, index: u32) -> u64;
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
                Ok(ValueRef::StringLength { ptr, offset }) => {
                    let len = input_get_length(ptr) as usize;
                    read_string(ptr + offset, len)
                }
                Ok(ValueRef::String { ptr, len }) => read_string(ptr, len),
                _ => None,
            },
        }
    }

    pub fn is_obj(&self) -> bool {
        match self {
            Value::NanBox(v) => matches!(v.try_decode(), Ok(ValueRef::Object { .. })),
        }
    }

    pub fn get_obj_prop(&self, prop: &str) -> Value {
        match self {
            Value::NanBox(v) => {
                let scope = unsafe {
                    shopify_function_input_get_obj_prop(v.to_bits(), prop.as_ptr(), prop.len())
                };
                Value::NanBox(NanBox::from_bits(scope))
            }
        }
    }

    pub fn is_array(&self) -> bool {
        match self {
            Value::NanBox(v) => matches!(v.try_decode(), Ok(ValueRef::Array { .. })),
        }
    }

    pub fn array_len(&self) -> Option<usize> {
        match self {
            Value::NanBox(v) => match v.try_decode() {
                Ok(ValueRef::Array { len, .. }) => Some(len),
                _ => None,
            },
        }
    }

    pub fn get_at_index(&self, index: u32) -> Value {
        match self {
            Value::NanBox(v) => {
                let scope = unsafe { shopify_function_input_get_at_index(v.to_bits(), index) };
                Value::NanBox(NanBox::from_bits(scope))
            }
        }
    }
}

fn read_string(ptr: usize, len: usize) -> Option<String> {
    let mut buf = vec![0; len];
    unsafe { shopify_function_input_read_utf8_str(ptr as _, buf.as_mut_ptr(), len) };
    Some(unsafe { String::from_utf8_unchecked(buf) })
}

pub fn input_get() -> Value {
    let val = unsafe { shopify_function_input_get() };
    Value::NanBox(NanBox::from_bits(val))
}

pub fn input_get_length(ptr: usize) -> u64 {
    unsafe { shopify_function_input_get_length(ptr) }
}
