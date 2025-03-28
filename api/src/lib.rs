use shopify_function_wasm_api_core::{
    read::{NanBox, Val, ValueRef},
    write::{WriteContext, WriteResult},
};

pub mod write;

#[link(wasm_import_module = "shopify_function_v0.1.0")]
extern "C" {
    // Read API.
    fn shopify_function_input_get() -> Val;
    fn shopify_function_input_get_val_len(scope: Val) -> usize;
    fn shopify_function_input_read_utf8_str(src: usize, out: *mut u8, len: usize);
    fn shopify_function_input_get_obj_prop(scope: Val, ptr: *const u8, len: usize) -> Val;
    fn shopify_function_input_get_at_index(scope: Val, index: usize) -> Val;

    // Write API.
    fn shopify_function_output_new() -> WriteContext;
    fn shopify_function_output_new_bool(context: WriteContext, bool: u32) -> WriteResult;
    fn shopify_function_output_new_null(context: WriteContext) -> WriteResult;
    fn shopify_function_output_finalize(context: WriteContext) -> WriteResult;
    fn shopify_function_output_new_i32(context: WriteContext, int: i32) -> WriteResult;
    fn shopify_function_output_new_f64(context: WriteContext, float: f64) -> WriteResult;
    fn shopify_function_output_new_utf8_str(
        context: WriteContext,
        ptr: *const u8,
        len: usize,
    ) -> WriteResult;
    fn shopify_function_output_new_object(context: WriteContext, len: usize) -> WriteResult;
    fn shopify_function_output_finish_object(context: WriteContext) -> WriteResult;
    fn shopify_function_output_new_array(context: WriteContext, len: usize) -> WriteResult;
    fn shopify_function_output_finish_array(context: WriteContext) -> WriteResult;
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
                    let len = if len as u64 == NanBox::MAX_VALUE_LENGTH {
                        unsafe { shopify_function_input_get_val_len(v.to_bits()) as usize }
                    } else {
                        len
                    };
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
                Ok(ValueRef::Array { len, .. }) => {
                    let len = if len as u64 == NanBox::MAX_VALUE_LENGTH {
                        unsafe { shopify_function_input_get_val_len(v.to_bits()) as usize }
                    } else {
                        len
                    };
                    Some(len)
                }
                _ => None,
            },
        }
    }

    pub fn get_at_index(&self, index: usize) -> Value {
        match self {
            Value::NanBox(v) => {
                let scope = unsafe { shopify_function_input_get_at_index(v.to_bits(), index) };
                Value::NanBox(NanBox::from_bits(scope))
            }
        }
    }
}

pub fn input_get() -> Value {
    let val = unsafe { shopify_function_input_get() };
    Value::NanBox(NanBox::from_bits(val))
}
