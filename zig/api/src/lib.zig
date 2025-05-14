const std = @import("std");
const core = @import("shopify_function_wasm_api_core");
pub const read = @import("read.zig");
pub const write = @import("write.zig");

pub const ContextError = error{
    NullPointer,
};

/// An identifier for an interned UTF-8 string
pub const InternedStringId = struct {
    id: core.InternedStringId,

    pub fn asUsize(self: InternedStringId) usize {
        return self.id;
    }
};

/// A mechanism for caching interned string IDs
pub const CachedInternedStringId = struct {
    value: []const u8,
    interned_string_id: std.atomic.Value(usize),
    context: std.atomic.Value(?*anyopaque),

    pub fn init(value: []const u8) CachedInternedStringId {
        return .{
            .value = value,
            .interned_string_id = std.atomic.Value(usize).init(std.math.maxInt(usize)),
            .context = std.atomic.Value(?*anyopaque).init(null),
        };
    }

    pub fn loadFromContext(self: *CachedInternedStringId, context: *Context) InternedStringId {
        return self.loadFromContextPtr(context.ptr);
    }

    pub fn loadFromValue(self: *CachedInternedStringId, value: *Value) InternedStringId {
        return self.loadFromContextPtr(value.context);
    }

    fn loadFromContextPtr(self: *CachedInternedStringId, context: core.ContextPtr) InternedStringId {
        if (self.context.load(.Unordered) != context) {
            const id = shopify_function_intern_utf8_str(
                context, 
                self.value.ptr, 
                self.value.len
            );
            self.interned_string_id.store(id, .Relaxed);
            self.context.store(context, .Relaxed);
        }
        return .{ .id = self.interned_string_id.load(.Relaxed) };
    }
};

/// A value read from the input
pub const Value = struct {
    context: core.ContextPtr,
    nan_box: core.read.NanBox,

    fn newChild(self: Value, nan_box: core.read.NanBox) Value {
        return .{
            .context = self.context,
            .nan_box = nan_box,
        };
    }

    pub fn internUtf8Str(self: Value, s: []const u8) InternedStringId {
        const id = shopify_function_intern_utf8_str(
            self.context, 
            s.ptr, 
            s.len
        );
        return .{ .id = id };
    }

    pub fn asBool(self: Value) ?bool {
        const value_ref = self.nan_box.tryDecode() catch return null;
        return switch (value_ref) {
            .Bool => |b| b,
            else => null,
        };
    }

    pub fn isNull(self: Value) bool {
        const value_ref = self.nan_box.tryDecode() catch return false;
        return switch (value_ref) {
            .Null => true,
            else => false,
        };
    }

    pub fn asNumber(self: Value) ?f64 {
        const value_ref = self.nan_box.tryDecode() catch return null;
        return switch (value_ref) {
            .Number => |n| n,
            else => null,
        };
    }

    pub fn asString(self: Value) ?[]const u8 {
        const value_ref = self.nan_box.tryDecode() catch return null;
        return switch (value_ref) {
            .String => |s| {
                const len = if (s.len == core.read.NanBox.MAX_VALUE_LENGTH)
                    shopify_function_input_get_val_len(self.context, self.nan_box.toBits())
                else
                    s.len;

                const buf = std.heap.page_allocator.alloc(u8, len) catch return null;
                shopify_function_input_read_utf8_str(
                    self.context,
                    s.ptr,
                    buf.ptr,
                    buf.len
                );
                return buf;
            },
            else => null,
        };
    }

    pub fn isObj(self: Value) bool {
        const value_ref = self.nan_box.tryDecode() catch return false;
        return switch (value_ref) {
            .Object => true,
            else => false,
        };
    }

    pub fn getObjProp(self: Value, prop: []const u8) Value {
        const scope = shopify_function_input_get_obj_prop(
            self.context,
            self.nan_box.toBits(),
            prop.ptr,
            prop.len
        );
        return self.newChild(core.read.NanBox.fromBits(scope));
    }

    pub fn getInternedObjProp(self: Value, interned_string_id: InternedStringId) Value {
        const scope = shopify_function_input_get_interned_obj_prop(
            self.context,
            self.nan_box.toBits(),
            interned_string_id.asUsize()
        );
        return self.newChild(core.read.NanBox.fromBits(scope));
    }

    pub fn isArray(self: Value) bool {
        const value_ref = self.nan_box.tryDecode() catch return false;
        return switch (value_ref) {
            .Array => true,
            else => false,
        };
    }

    pub fn arrayLen(self: Value) ?usize {
        const value_ref = self.nan_box.tryDecode() catch return null;
        return switch (value_ref) {
            .Array => |a| {
                if (a.len == core.read.NanBox.MAX_VALUE_LENGTH) {
                    return shopify_function_input_get_val_len(
                        self.context,
                        self.nan_box.toBits()
                    );
                } else {
                    return a.len;
                }
            },
            else => null,
        };
    }

    pub fn objLen(self: Value) ?usize {
        const value_ref = self.nan_box.tryDecode() catch return null;
        return switch (value_ref) {
            .Object => |o| {
                if (o.len == core.read.NanBox.MAX_VALUE_LENGTH) {
                    return shopify_function_input_get_val_len(
                        self.context,
                        self.nan_box.toBits()
                    );
                } else {
                    return o.len;
                }
            },
            else => null,
        };
    }

    pub fn getAtIndex(self: Value, index: usize) Value {
        const scope = shopify_function_input_get_at_index(
            self.context,
            self.nan_box.toBits(),
            index
        );
        return self.newChild(core.read.NanBox.fromBits(scope));
    }

    pub fn getObjKeyAtIndex(self: Value, index: usize) ?[]const u8 {
        const value_ref = self.nan_box.tryDecode() catch return null;
        return switch (value_ref) {
            .Object => {
                const scope = shopify_function_input_get_obj_key_at_index(
                    self.context,
                    self.nan_box.toBits(),
                    index
                );
                const value = self.newChild(core.read.NanBox.fromBits(scope));
                return value.asString();
            },
            else => null,
        };
    }

    pub fn asError(self: Value) ?core.read.ErrorCode {
        const value_ref = self.nan_box.tryDecode() catch return null;
        return switch (value_ref) {
            .Error => |e| e,
            else => null,
        };
    }
};

/// A context for reading and writing values
pub const Context = struct {
    ptr: core.ContextPtr,

    pub fn init() Context {
        return .{
            .ptr = shopify_function_context_new(),
        };
    }

    pub fn inputGet(self: Context) !Value {
        const val = shopify_function_input_get(self.ptr);
        if (self.ptr == null) {
            return ContextError.NullPointer;
        }
        return Value{
            .context = self.ptr,
            .nan_box = core.read.NanBox.fromBits(val),
        };
    }

    pub fn internUtf8Str(self: Context, s: []const u8) InternedStringId {
        const id = shopify_function_intern_utf8_str(
            self.ptr,
            s.ptr,
            s.len
        );
        return .{ .id = id };
    }

    // Write API functions
    pub fn writeBool(self: *Context, value: bool) !void {
        const result = shopify_function_output_new_bool(self.ptr, @intFromBool(value));
        return write.mapResult(result);
    }

    pub fn writeNull(self: *Context) !void {
        const result = shopify_function_output_new_null(self.ptr);
        return write.mapResult(result);
    }

    pub fn writeI32(self: *Context, value: i32) !void {
        const result = shopify_function_output_new_i32(self.ptr, value);
        return write.mapResult(result);
    }

    pub fn writeF64(self: *Context, value: f64) !void {
        const result = shopify_function_output_new_f64(self.ptr, value);
        return write.mapResult(result);
    }

    pub fn writeUtf8Str(self: *Context, value: []const u8) !void {
        const result = shopify_function_output_new_utf8_str(
            self.ptr,
            value.ptr,
            value.len
        );
        return write.mapResult(result);
    }

    pub fn writeInternedUtf8Str(self: *Context, id: InternedStringId) !void {
        const result = shopify_function_output_new_interned_utf8_str(
            self.ptr,
            id.asUsize()
        );
        return write.mapResult(result);
    }

    pub fn writeObject(
        self: *Context, 
        writer: *const fn(*Context) anyerror!void,
        len: usize
    ) !void {
        const result = shopify_function_output_new_object(self.ptr, len);
        try write.mapResult(result);
        try writer(self);
        const finish_result = shopify_function_output_finish_object(self.ptr);
        return write.mapResult(finish_result);
    }

    pub fn writeArray(
        self: *Context, 
        writer: *const fn(*Context) anyerror!void,
        len: usize
    ) !void {
        const result = shopify_function_output_new_array(self.ptr, len);
        try write.mapResult(result);
        try writer(self);
        const finish_result = shopify_function_output_finish_array(self.ptr);
        return write.mapResult(finish_result);
    }

    pub fn finalizeOutput(self: Context) !void {
        const result = shopify_function_output_finalize(self.ptr);
        return write.mapResult(result);
    }
};

// External functions declaration from shopify_function_v1 module
extern "shopify_function_v1" fn shopify_function_context_new() core.ContextPtr;
extern "shopify_function_v1" fn shopify_function_input_get(context: core.ContextPtr) core.read.Val;
extern "shopify_function_v1" fn shopify_function_input_get_val_len(context: core.ContextPtr, scope: core.read.Val) usize;
extern "shopify_function_v1" fn shopify_function_input_read_utf8_str(
    context: core.ContextPtr,
    src: usize,
    out: [*]u8,
    len: usize,
) void;
extern "shopify_function_v1" fn shopify_function_input_get_obj_prop(
    context: core.ContextPtr,
    scope: core.read.Val,
    ptr: [*]const u8,
    len: usize,
) core.read.Val;
extern "shopify_function_v1" fn shopify_function_input_get_interned_obj_prop(
    context: core.ContextPtr,
    scope: core.read.Val,
    interned_string_id: core.InternedStringId,
) core.read.Val;
extern "shopify_function_v1" fn shopify_function_input_get_at_index(
    context: core.ContextPtr,
    scope: core.read.Val,
    index: usize,
) core.read.Val;
extern "shopify_function_v1" fn shopify_function_input_get_obj_key_at_index(
    context: core.ContextPtr,
    scope: core.read.Val,
    index: usize,
) core.read.Val;
extern "shopify_function_v1" fn shopify_function_output_new_bool(
    context: core.ContextPtr,
    bool: u32,
) core.write.WriteResult;
extern "shopify_function_v1" fn shopify_function_output_new_null(context: core.ContextPtr) core.write.WriteResult;
extern "shopify_function_v1" fn shopify_function_output_finalize(context: core.ContextPtr) core.write.WriteResult;
extern "shopify_function_v1" fn shopify_function_output_new_i32(
    context: core.ContextPtr,
    int: i32,
) core.write.WriteResult;
extern "shopify_function_v1" fn shopify_function_output_new_f64(
    context: core.ContextPtr,
    float: f64,
) core.write.WriteResult;
extern "shopify_function_v1" fn shopify_function_output_new_utf8_str(
    context: core.ContextPtr,
    ptr: [*]const u8,
    len: usize,
) core.write.WriteResult;
extern "shopify_function_v1" fn shopify_function_output_new_interned_utf8_str(
    context: core.ContextPtr,
    id: core.InternedStringId,
) core.write.WriteResult;
extern "shopify_function_v1" fn shopify_function_output_new_object(
    context: core.ContextPtr,
    len: usize,
) core.write.WriteResult;
extern "shopify_function_v1" fn shopify_function_output_finish_object(context: core.ContextPtr) core.write.WriteResult;
extern "shopify_function_v1" fn shopify_function_output_new_array(
    context: core.ContextPtr,
    len: usize,
) core.write.WriteResult;
extern "shopify_function_v1" fn shopify_function_output_finish_array(context: core.ContextPtr) core.write.WriteResult;
extern "shopify_function_v1" fn shopify_function_intern_utf8_str(
    context: core.ContextPtr,
    ptr: [*]const u8,
    len: usize,
) usize;