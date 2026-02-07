const sf = @import("shopify_function");

fn collectErrors(cart: sf.Value) bool {
    if (cart.objLen() == null) return false;

    const lines = cart.getObjProp("lines");
    const lines_len = lines.arrayLen() orelse return false;

    for (0..lines_len) |i| {
        const line = lines.getAtIndex(@intCast(i));
        if (line.objLen() != null) {
            const quantity = line.getObjProp("quantity");
            if (quantity.asNumber()) |q| {
                if (q > 1.0) return true;
            }
        }
    }

    return false;
}

export fn _start() void {
    const input = sf.inputGet();
    const cart = input.getObjProp("cart");
    const has_error = collectErrors(cart);

    // {"errors": [...]}
    sf.outputObject(1) catch return;
    sf.outputString("errors") catch return;

    if (has_error) {
        sf.outputArray(1) catch return;
        // {"localizedMessage": "...", "target": "$.cart"}
        sf.outputObject(2) catch return;
        sf.outputString("localizedMessage") catch return;
        sf.outputString("Not possible to order more than one of each") catch return;
        sf.outputString("target") catch return;
        sf.outputString("$.cart") catch return;
        sf.outputFinishObject() catch return;
        sf.outputFinishArray() catch return;
    } else {
        sf.outputArray(0) catch return;
        sf.outputFinishArray() catch return;
    }

    sf.outputFinishObject() catch return;
}
