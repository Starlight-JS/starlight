/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */
Math.max = function (...values) {
    var max = -Infinity;
    for (var i = 0; i < values.length; i++) {
        if (isNaN(values[i])) {
            return values[i];
        } else if (values[i] > max || (values[i] == 0.0 && max == 0.0)) {
            max = values[i];
        }
    }
    return max;
}

Math.min = function (...values) {
    var min = Infinity;
    for (var i = 0; i < values.length; i++) {
        if (isNaN(values[i])) {
            return values[i]
        } else if (values[i] < min || (values[i] == 0.0 && min == 0.0)) {
            min = values[i];
        }
    }
    return min;
}