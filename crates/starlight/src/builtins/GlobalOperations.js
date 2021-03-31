let toint = function ___toIntegerOrInfinity(target) {
    "use strict";
    let number_value = +target;
    if (number_value !== number_value || !number_value) {
        return 0;
    }
    return ___trunc(number_value);
}

Object.defineProperty(globalThis, "___toIntegerOrInfinity", {
    value: ___toIntegerOrInfinity,
    writable: false,
    configurable: false,
    enumerable: false
});

let toLength = function ___toLength(target) {
    "use strict";
    let length = ___toIntegerOrInfinity(target);

    return +length;
}

Object.defineProperty(
    globalThis, "___toLength",
    {
        value: toLength,
        writable: false,
        configurable: false,
        enumerable: false
    }
);
let ___toObject = function ___toObject(target, error) {
    if (target === null || target === undefined) {
        throw new TypeError(error);
    }

    return Object(target);
}

Object.defineProperty(globalThis, "___toObject", {
    value: ___toObject,
    writable: false,
    configurable: false,
    enumerable: false
});


let assert = function ___assert(cond) {
    if (!cond)
        throw "Assertion failed";
}

Object.defineProperty(globalThis, "___assert", {
    value: ___assert,
    writable: false,
    configurable: false,
    enumerable: false
});
