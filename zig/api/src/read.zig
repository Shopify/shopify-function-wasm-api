const std = @import("std");
const Value = @import("lib.zig").Value;

pub const Error = error{
    InvalidType,
};

/// Trait-like interface for types that can be deserialized from a Value
pub fn Deserialize(comptime T: type) type {
    return struct {
        pub fn deserialize(value: *const Value) Error!T {
            return switch (T) {
                Value => value.*,
                bool => value.asBool() orelse return Error.InvalidType,
                i32 => blk: {
                    const num = value.asNumber() orelse return Error.InvalidType;
                    if (std.math.trunc(num) == num and 
                        num >= @as(f64, @floatFromInt(std.math.minInt(i32))) and 
                        num <= @as(f64, @floatFromInt(std.math.maxInt(i32)))) {
                        break :blk @as(i32, @intFromFloat(num));
                    }
                    return Error.InvalidType;
                },
                f64 => value.asNumber() orelse return Error.InvalidType,
                []const u8, []u8 => value.asString() orelse return Error.InvalidType,
                ?T => {
                    if (value.isNull()) {
                        return null;
                    } else {
                        return @This().deserialize(value);
                    }
                },
                []T => {
                    if (!value.isArray()) {
                        return Error.InvalidType;
                    }
                    const len = value.arrayLen() orelse return Error.InvalidType;
                    var list = std.ArrayList(T).init(std.heap.page_allocator);
                    defer list.deinit();
                    try list.ensureTotalCapacity(len);
                    var i: usize = 0;
                    while (i < len) : (i += 1) {
                        const item = value.getAtIndex(i);
                        const value_t = try @This().deserialize(&item);
                        try list.append(value_t);
                    }
                    return list.toOwnedSlice();
                },
                std.ArrayList(T) => {
                    if (!value.isArray()) {
                        return Error.InvalidType;
                    }
                    const len = value.arrayLen() orelse return Error.InvalidType;
                    var list = std.ArrayList(T).init(std.heap.page_allocator);
                    try list.ensureTotalCapacity(len);
                    var i: usize = 0;
                    while (i < len) : (i += 1) {
                        const item = value.getAtIndex(i);
                        const value_t = try @This().deserialize(&item);
                        try list.append(value_t);
                    }
                    return list;
                },
                else => @compileError("Type " ++ @typeName(T) ++ " does not implement Deserialize"),
            };
        }
    };
}