const std = @import("std");
const sf = @import("shopify_function_wasm_api");
const Context = sf.Context;
const Value = sf.Value;

// Global used to communicate errors between functions
var global_errors: ?std.ArrayList([]const u8) = null;

pub fn main() !void {
    var context = Context.init();
    
    const input = try context.inputGet();
    const cart = input.getObjProp("cart");
    
    const errors = collectErrors(&cart);
    global_errors = errors;
    defer {
        if (global_errors) |*e| {
            e.deinit();
        }
    }
    
    // Write the response object
    try context.writeObject(writeResponseWithErrors, 1);
    try context.finalizeOutput();
}

// Function to write the response with error messages
fn writeResponseWithErrors(ctx: *Context) !void {
    try ctx.writeUtf8Str("errors");
    
    const items_len = if (global_errors) |errors| errors.items.len else 0;
    try ctx.writeArray(writeArrayWithErrorMessages, items_len);
}

// Function to write the array of error messages
fn writeArrayWithErrorMessages(array_ctx: *Context) !void {
    if (global_errors) |errors| {
        for (errors.items) |_| {
            try array_ctx.writeObject(writeErrorObject, 2);
        }
    }
}

// Function to write a single error object with message and target
fn writeErrorObject(error_ctx: *Context) !void {
    if (global_errors) |errors| {
        if (errors.items.len > 0) {
            try error_ctx.writeUtf8Str("localizedMessage");
            try error_ctx.writeUtf8Str(errors.items[0]);
            
            try error_ctx.writeUtf8Str("target");
            try error_ctx.writeUtf8Str("$.cart");
        }
    }
}

// Helper function to collect errors for lines with quantity > 1
fn collectErrors(cart: *const Value) std.ArrayList([]const u8) {
    var errors = std.ArrayList([]const u8).init(std.heap.page_allocator);
    
    if (!cart.isObj()) {
        return errors;
    }
    
    const lines = cart.getObjProp("lines");
    
    if (!lines.isArray()) {
        return errors;
    }
    
    if (lines.arrayLen()) |lines_len| {
        var i: usize = 0;
        while (i < lines_len) : (i += 1) {
            const line = lines.getAtIndex(i);
            if (line.isObj()) {
                const quantity = line.getObjProp("quantity");
                if (quantity.asNumber()) |q| {
                    if (q > 1.0) {
                        errors.append("Not possible to order more than one of each") catch break;
                        break; // Only one error message needed
                    }
                }
            }
        }
    }
    
    return errors;
}