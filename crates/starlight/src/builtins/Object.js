/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */
Object.defineProperties = function defineProperties(object, properties) {
    let object_ = object;
    let properties_ = properties;

    Object.keys(properties).forEach(function (property) {
        if (property !== '__proto__') {
            Object.defineProperty(object_, property, properties_[property]);

        }
    });
    return object;
}

Object.defineProperty(Object, '___defineProperties___', {
    value: Object.defineProperties,
    writable: false,
    enumerable: false,
    configurable: false
})

Object.is = function (x, y) {
    // SameValue algorithm
    if (x === y) { // Steps 1-5, 7-10
        // Steps 6.b-6.e: +0 != -0
        return x !== 0 || 1 / x === 1 / y;
    } else {
        // Step 6.a: NaN == NaN
        return x !== x && y !== y;
    }
};