const std = @import("std");

/// A type alias to represent raw NaN-boxed values.
pub const Val = if (@sizeOf(usize) == 8) u128 else u64;

/// Tag type for NanBox values
pub const Tag = enum(u8) {
    Null = 0,
    Bool = 1,
    Number = 2,
    String = 3,
    Object = 4,
    Array = 5,
    Error = NanBox.MAX_TAG_VALUE,
};

/// Error codes that can be returned from API operations
pub const ErrorCode = enum(usize) {
    DecodeError = 0,
    NotAnObject = 1,
    ByteArrayOutOfBounds = 2,
    ReadError = 3,
    NotAnArray = 4,
    IndexOutOfBounds = 5,
    NotIndexable = 6,
};

/// Values are represented as NaN-boxed values.
pub const NanBox = struct {
    bits: Val,

    // Constants matching the Rust implementation
    pub const F64_OFFSET: u8 = @bitSizeOf(Val) - 64;
    pub const PAYLOAD_SIZE: u8 = 50 + F64_OFFSET;
    pub const MANTISSA_SIZE: u8 = 52;
    pub const QUIET_NAN_SIZE: u8 = MANTISSA_SIZE + F64_OFFSET - PAYLOAD_SIZE;
    pub const EXPONENT_SIZE: u8 = 11;
    pub const NAN_MASK: Val = ((1 << (QUIET_NAN_SIZE + EXPONENT_SIZE)) - 1) << PAYLOAD_SIZE;
    pub const PAYLOAD_MASK: Val = ~(NAN_MASK | (1 << (MANTISSA_SIZE + EXPONENT_SIZE + F64_OFFSET)));
    pub const TAG_SIZE: u8 = 4;
    pub const MAX_TAG_VALUE: u8 = (1 << TAG_SIZE) - 1;
    pub const TAG_MASK: Val = @as(Val, MAX_TAG_VALUE) << VALUE_SIZE;
    pub const VALUE_SIZE: u8 = PAYLOAD_SIZE - TAG_SIZE;
    pub const VALUE_ENCODING_SIZE: u8 = @bitSizeOf(usize);
    pub const VALUE_LENGTH_SIZE: u8 = VALUE_SIZE - VALUE_ENCODING_SIZE;
    pub const MAX_VALUE_LENGTH: usize = (1 << VALUE_LENGTH_SIZE) - 1;
    pub const VALUE_MASK: Val = PAYLOAD_MASK & ~TAG_MASK;
    pub const POINTER_MASK: Val = (1 << VALUE_ENCODING_SIZE) - 1;

    /// Create a new NanBox from a raw value
    pub fn fromBits(bits: Val) NanBox {
        return .{ .bits = bits };
    }

    /// Get the raw bits from the NanBox
    pub fn toBits(self: NanBox) Val {
        return self.bits;
    }

    /// Create a new boolean NanBox
    pub fn boolean(val: bool) NanBox {
        return encode(@intFromBool(val), 0, .Bool);
    }

    /// Create a null NanBox
    pub fn null_() NanBox {
        return encode(0, 0, .Null);
    }

    /// Create a new number NanBox
    pub fn number(val: f64) NanBox {
        std.debug.assert(!std.math.isNan(val));
        return .{ .bits = (@as(Val, @bitCast(val)) << F64_OFFSET) };
    }

    /// Create a new string NanBox
    pub fn string(ptr: usize, len: usize) NanBox {
        return encode(ptr, len, .String);
    }

    /// Create a new object NanBox
    pub fn obj(ptr: usize, len: usize) NanBox {
        return encode(ptr, len, .Object);
    }

    /// Create a new error NanBox
    pub fn error_(code: ErrorCode) NanBox {
        return encode(@intFromEnum(code), 0, .Error);
    }

    /// Create a new array NanBox
    pub fn array(ptr: usize, len: usize) NanBox {
        return encode(ptr, len, .Array);
    }

    fn encode(ptr: usize, len: usize, tag: Tag) NanBox {
        const trimmed_len = @min(len, MAX_VALUE_LENGTH);
        const val = (@as(Val, trimmed_len) << VALUE_ENCODING_SIZE) | (@as(Val, ptr) & POINTER_MASK);
        return .{ .bits = NAN_MASK | (@as(Val, @intFromEnum(tag)) << VALUE_SIZE) | val };
    }

    /// Helper to extract the tag
    fn getTag(self: NanBox) !Tag {
        const tag_val = (self.bits & PAYLOAD_MASK) >> VALUE_SIZE;
        // Convert to Tag enum
        return std.meta.intToEnum(Tag, @as(u8, @truncate(tag_val)));
    }

    /// Try to decode a NanBox into a ValueRef
    pub fn tryDecode(self: NanBox) !ValueRef {
        // Check if it's a number (not NaN)
        if (self.bits & NAN_MASK != NAN_MASK) {
            if (@sizeOf(usize) == 4) {
                const value = self.bits;
                return ValueRef{ .Number = @bitCast(value) };
            } else {
                const value = @as(u64, @truncate(self.bits >> F64_OFFSET));
                return ValueRef{ .Number = @bitCast(value) };
            }
        }

        const val = self.bits & VALUE_MASK;
        const ptr = val & POINTER_MASK;
        const len = val >> VALUE_ENCODING_SIZE;

        const uptr = @as(usize, @truncate(ptr));
        const ulen = @as(usize, @truncate(len));

        const tag = try self.getTag();

        return switch (tag) {
            .Bool => ValueRef{ .Bool = uptr != 0 },
            .Null => ValueRef.Null,
            .Number => unreachable, // Number values aren't NaN-boxed
            .Array => ValueRef{ .Array = .{ .ptr = uptr, .len = ulen } },
            .String => ValueRef{ .String = .{ .ptr = uptr, .len = ulen } },
            .Object => ValueRef{ .Object = .{ .ptr = uptr, .len = ulen } },
            .Error => ValueRef{ .Error = @as(ErrorCode, @enumFromInt(uptr)) },
        };
    }
};

/// An unwrapped representation of a NaN-boxed value
pub const ValueRef = union(enum) {
    Null,
    Bool: bool,
    Number: f64,
    String: struct { ptr: usize, len: usize },
    Object: struct { ptr: usize, len: usize },
    Array: struct { ptr: usize, len: usize },
    Error: ErrorCode,
};