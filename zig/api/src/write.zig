const std = @import("std");
const Context = @import("lib.zig").Context;
const core = @import("shopify_function_wasm_api_core");

pub const Error = error{
    IoError,
    ExpectedKey,
    ObjectLengthError,
    ValueAlreadyWritten,
    NotAnObject,
    ValueNotFinished,
    ArrayLengthError,
    NotAnArray,
};

pub fn mapResult(result: core.write.WriteResult) Error!void {
    return switch (result) {
        .Ok => {},
        .IoError => Error.IoError,
        .ExpectedKey => Error.ExpectedKey,
        .ObjectLengthError => Error.ObjectLengthError,
        .ValueAlreadyWritten => Error.ValueAlreadyWritten,
        .NotAnObject => Error.NotAnObject,
        .ValueNotFinished => Error.ValueNotFinished,
        .ArrayLengthError => Error.ArrayLengthError,
        .NotAnArray => Error.NotAnArray,
    };
}

pub fn Serialize(comptime T: type) type {
    return struct {
        pub fn serialize(value: T, context: *Context) Error!void {
            return switch (@TypeOf(value)) {
                bool => context.writeBool(value),
                void => context.writeNull(),
                i32 => context.writeI32(value),
                f64 => context.writeF64(value),
                []const u8, []u8 => context.writeUtf8Str(value),
                ?T => if (value) |v| try serialize(v, context) else try context.writeNull(),
                []T, std.ArrayList(T) => {
                    const len = switch (@TypeOf(value)) {
                        []T => value.len,
                        std.ArrayList(T) => value.items.len,
                        else => unreachable,
                    };
                    
                    return try context.writeArray(
                        struct {
                            const Closure = @This();
                            v: @TypeOf(value),
                            
                            pub fn write(self: *const Closure, ctx: *Context) !void {
                                const items = switch (@TypeOf(self.v)) {
                                    []T => self.v,
                                    std.ArrayList(T) => self.v.items,
                                    else => unreachable,
                                };
                                
                                for (items) |item| {
                                    try serialize(item, ctx);
                                }
                            }
                        }.write,
                        len
                    );
                },
                else => @compileError("Type " ++ @typeName(T) ++ " does not implement Serialize"),
            };
        }
    };
}