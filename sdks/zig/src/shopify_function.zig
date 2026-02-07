// Shopify Function WASM API - Zig bindings
//
// NaN-box layout for wasm32 (Val = i64):
//   bits 50-62: NaN pattern (0x7FFC000000000000)
//   bits 46-49: tag (4 bits)
//   bits 32-45: value length (14 bits)
//   bits 0-31:  value/pointer (32 bits)
//
// Numbers (f64) are stored as raw IEEE 754 bits, NOT NaN-boxed.

const std = @import("std");

pub const Val = i64;
pub const WriteResult = i32;
pub const InternedStringId = u32;

// NaN-box constants
const NAN_MASK: u64 = 0x7FFC000000000000;
const PAYLOAD_MASK: u64 = 0x0003FFFFFFFFFFFF;
const TAG_MASK: u64 = 0x0003C00000000000;
const VALUE_MASK: u64 = 0x00003FFFFFFFFFFF;
const POINTER_MASK: u64 = 0x00000000FFFFFFFF;
const TAG_SHIFT: u6 = 46;
const VALUE_ENCODING_SIZE: u6 = 32;
const MAX_VALUE_LENGTH: u32 = 16383;

pub const ValueTag = enum(u4) {
    null = 0,
    boolean = 1,
    number = 2,
    string = 3,
    object = 4,
    array = 5,
    err = 15,
};

pub const WriteError = error{
    IoError,
    ExpectedKey,
    ObjectLengthError,
    ValueAlreadyWritten,
    NotAnObject,
    ValueNotFinished,
    ArrayLengthError,
    NotAnArray,
};

fn writeResultToError(result: WriteResult) WriteError!void {
    return switch (result) {
        0 => {},
        1 => WriteError.IoError,
        2 => WriteError.ExpectedKey,
        3 => WriteError.ObjectLengthError,
        4 => WriteError.ValueAlreadyWritten,
        5 => WriteError.NotAnObject,
        6 => WriteError.ValueNotFinished,
        7 => WriteError.ArrayLengthError,
        8 => WriteError.NotAnArray,
        else => WriteError.IoError,
    };
}

// --- Extern declarations ---

const sf = struct {
    extern "shopify_function_v2" fn shopify_function_input_get() callconv(.C) Val;
    extern "shopify_function_v2" fn shopify_function_input_get_val_len(scope: Val) callconv(.C) u32;
    extern "shopify_function_v2" fn shopify_function_input_read_utf8_str(src: u32, out: [*]u8, len: u32) callconv(.C) void;
    extern "shopify_function_v2" fn shopify_function_input_get_obj_prop(scope: Val, ptr: [*]const u8, len: u32) callconv(.C) Val;
    extern "shopify_function_v2" fn shopify_function_input_get_interned_obj_prop(scope: Val, id: InternedStringId) callconv(.C) Val;
    extern "shopify_function_v2" fn shopify_function_input_get_at_index(scope: Val, index: u32) callconv(.C) Val;
    extern "shopify_function_v2" fn shopify_function_input_get_obj_key_at_index(scope: Val, index: u32) callconv(.C) Val;
    extern "shopify_function_v2" fn shopify_function_output_new_bool(value: u32) callconv(.C) WriteResult;
    extern "shopify_function_v2" fn shopify_function_output_new_null() callconv(.C) WriteResult;
    extern "shopify_function_v2" fn shopify_function_output_new_i32(value: i32) callconv(.C) WriteResult;
    extern "shopify_function_v2" fn shopify_function_output_new_f64(value: f64) callconv(.C) WriteResult;
    extern "shopify_function_v2" fn shopify_function_output_new_utf8_str(ptr: [*]const u8, len: u32) callconv(.C) WriteResult;
    extern "shopify_function_v2" fn shopify_function_output_new_interned_utf8_str(id: InternedStringId) callconv(.C) WriteResult;
    extern "shopify_function_v2" fn shopify_function_output_new_object(len: u32) callconv(.C) WriteResult;
    extern "shopify_function_v2" fn shopify_function_output_finish_object() callconv(.C) WriteResult;
    extern "shopify_function_v2" fn shopify_function_output_new_array(len: u32) callconv(.C) WriteResult;
    extern "shopify_function_v2" fn shopify_function_output_finish_array() callconv(.C) WriteResult;
    extern "shopify_function_v2" fn shopify_function_intern_utf8_str(ptr: [*]const u8, len: u32) callconv(.C) InternedStringId;
    extern "shopify_function_v2" fn shopify_function_log_new_utf8_str(ptr: [*]const u8, len: u32) callconv(.C) void;
};

// --- Value type ---

pub const Value = struct {
    raw: Val,

    pub fn getTag(self: Value) ValueTag {
        const bits: u64 = @bitCast(self.raw);
        if ((bits & NAN_MASK) != NAN_MASK) {
            return .number;
        }
        const tag_bits: u4 = @intCast((bits & TAG_MASK) >> TAG_SHIFT);
        return @enumFromInt(tag_bits);
    }

    pub fn isNull(self: Value) bool {
        return self.getTag() == .null;
    }

    pub fn asBool(self: Value) ?bool {
        if (self.getTag() != .boolean) return null;
        const bits: u64 = @bitCast(self.raw);
        return (bits & POINTER_MASK) != 0;
    }

    pub fn asNumber(self: Value) ?f64 {
        if (self.getTag() != .number) return null;
        return @bitCast(self.raw);
    }

    fn inlineLen(self: Value) u32 {
        const bits: u64 = @bitCast(self.raw);
        return @intCast((bits & VALUE_MASK) >> VALUE_ENCODING_SIZE);
    }

    fn ptr(self: Value) u32 {
        const bits: u64 = @bitCast(self.raw);
        return @intCast(bits & POINTER_MASK);
    }

    pub fn stringLen(self: Value) u32 {
        const len = self.inlineLen();
        if (len < MAX_VALUE_LENGTH) return len;
        return sf.shopify_function_input_get_val_len(self.raw);
    }

    pub fn readString(self: Value, buf: []u8) void {
        sf.shopify_function_input_read_utf8_str(self.ptr(), buf.ptr, @intCast(buf.len));
    }

    pub fn objLen(self: Value) ?u32 {
        if (self.getTag() != .object) return null;
        const len = self.inlineLen();
        if (len < MAX_VALUE_LENGTH) return len;
        return sf.shopify_function_input_get_val_len(self.raw);
    }

    pub fn arrayLen(self: Value) ?u32 {
        if (self.getTag() != .array) return null;
        const len = self.inlineLen();
        if (len < MAX_VALUE_LENGTH) return len;
        return sf.shopify_function_input_get_val_len(self.raw);
    }

    pub fn getAtIndex(self: Value, index: u32) Value {
        return .{ .raw = sf.shopify_function_input_get_at_index(self.raw, index) };
    }

    pub fn getObjKeyAtIndex(self: Value, index: u32) Value {
        return .{ .raw = sf.shopify_function_input_get_obj_key_at_index(self.raw, index) };
    }

    pub fn getObjProp(self: Value, name: []const u8) Value {
        return .{ .raw = sf.shopify_function_input_get_obj_prop(self.raw, name.ptr, @intCast(name.len)) };
    }

    pub fn getInternedObjProp(self: Value, id: InternedStringId) Value {
        return .{ .raw = sf.shopify_function_input_get_interned_obj_prop(self.raw, id) };
    }
};

// --- Input ---

pub fn inputGet() Value {
    return .{ .raw = sf.shopify_function_input_get() };
}

// --- Output helpers ---

pub fn outputBool(value: bool) WriteError!void {
    return writeResultToError(sf.shopify_function_output_new_bool(if (value) 1 else 0));
}

pub fn outputNull() WriteError!void {
    return writeResultToError(sf.shopify_function_output_new_null());
}

pub fn outputI32(value: i32) WriteError!void {
    return writeResultToError(sf.shopify_function_output_new_i32(value));
}

pub fn outputF64(value: f64) WriteError!void {
    return writeResultToError(sf.shopify_function_output_new_f64(value));
}

pub fn outputString(data: []const u8) WriteError!void {
    return writeResultToError(sf.shopify_function_output_new_utf8_str(data.ptr, @intCast(data.len)));
}

pub fn outputInternedString(id: InternedStringId) WriteError!void {
    return writeResultToError(sf.shopify_function_output_new_interned_utf8_str(id));
}

pub fn outputObject(len: u32) WriteError!void {
    return writeResultToError(sf.shopify_function_output_new_object(len));
}

pub fn outputFinishObject() WriteError!void {
    return writeResultToError(sf.shopify_function_output_finish_object());
}

pub fn outputArray(len: u32) WriteError!void {
    return writeResultToError(sf.shopify_function_output_new_array(len));
}

pub fn outputFinishArray() WriteError!void {
    return writeResultToError(sf.shopify_function_output_finish_array());
}

pub fn internString(data: []const u8) InternedStringId {
    return sf.shopify_function_intern_utf8_str(data.ptr, @intCast(data.len));
}

pub fn log(data: []const u8) void {
    sf.shopify_function_log_new_utf8_str(data.ptr, @intCast(data.len));
}
