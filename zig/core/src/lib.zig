pub const read = @import("read.zig");
pub const write = @import("write.zig");

/// The context used for serialization
pub const ContextPtr = ?*anyopaque;

pub const InternedStringId = usize;