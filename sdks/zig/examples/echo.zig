const sf = @import("shopify_function");

var string_buf: [65536]u8 = undefined;

fn echoValue(val: sf.Value) void {
    switch (val.getTag()) {
        .null => sf.outputNull() catch return,
        .boolean => sf.outputBool(val.asBool().?) catch return,
        .number => {
            const num = val.asNumber().?;
            const truncated = @trunc(num);
            if (truncated == num and num >= -2147483648.0 and num <= 2147483647.0) {
                sf.outputI32(@intFromFloat(num)) catch return;
            } else {
                sf.outputF64(num) catch return;
            }
        },
        .string => {
            const len = val.stringLen();
            const buf = string_buf[0..len];
            val.readString(buf);
            sf.outputString(buf) catch return;
        },
        .object => {
            const len = val.objLen().?;
            sf.outputObject(len) catch return;
            for (0..len) |i| {
                // Write key
                const key = val.getObjKeyAtIndex(@intCast(i));
                const key_len = key.stringLen();
                const key_buf = string_buf[0..key_len];
                key.readString(key_buf);
                sf.outputString(key_buf) catch return;

                // Write value
                const child = val.getAtIndex(@intCast(i));
                echoValue(child);
            }
            sf.outputFinishObject() catch return;
        },
        .array => {
            const len = val.arrayLen().?;
            sf.outputArray(len) catch return;
            for (0..len) |i| {
                const child = val.getAtIndex(@intCast(i));
                echoValue(child);
            }
            sf.outputFinishArray() catch return;
        },
        .err => sf.outputNull() catch return,
    }
}

export fn _start() void {
    const input = sf.inputGet();
    echoValue(input);
}
