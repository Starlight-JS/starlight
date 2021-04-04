globalThis.FFI.void = 0
globalThis.FFI.pointer = 1
globalThis.FFI.f64 = 2
globalThis.FFI.f32 = 3
globalThis.FFI.i8 = 4
globalThis.FFI.i16 = 5
globalThis.FFI.i32 = 6
globalThis.FFI.i64 = 7
globalThis.FFI.u8 = 8
globalThis.FFI.u16 = 9
globalThis.FFI.u32 = 10
globalThis.FFI.u64 = 11
globalThis.FFI.cstring = 12

FFI.toFFItype = function (val) {
    let ty = typeof val;
    if (ty == "string") {
        return FFI.cstring;
    } else if (ty == "number") {
        return FFI.f64;
    } else if (ty == "undefined") {
        return FFI.void;
    } else {
        throw "Todo";
    }
}
// allows to create CFunction with variadic arguments. 
CFunction.create = function cnew(library, name, args, ret, variadic) {
    if (!variadic) {
        return CFunction.attach(library, name, args, ret);
    } else {
        let real_args = args;
        return {
            call: function call(...args) {
                let vargs = []
                let types = []
                for (let i = 0; i < real_args.length; i += 1) {
                    vargs.push(args[i]);
                    types.push(real_args[i]);
                }
                for (let i = real_args.length; i < args.length; i += 1) {
                    vargs.push(args[i]);
                    types.push(FFI.toFFItype(args[i]));
                }

                let cfunc = CFunction.attach(library, name, types, ret);

                return cfunc.call(vargs);
            }
        }
    }
}