/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */
function ___StringIterator(string) {
    this.stringIteratorFieldIndex = 0;
    this.stringIteratorIteratedString = string;
}

___StringIterator.prototype.next = function next() {
    var done = true;
    var value = undefined;
    var position = this.stringIteratorFieldIndex;
    if (position !== -1) {
        var string = this.stringIteratorIteratedString;
        var length = string.length >>> 0;
        if (position >= length) {
            this.stringIteratorFieldIndex = -1;
        } else {
            done = false;
            var first = string.charCodeAt(position);
            if (first < 0xD800 || first > 0xDBFF || position + 1 === length)
                value = string[position];
            else {
                var second = string.charCodeAt(position + 1);
                if (second < 0xDC00 || second > 0XDFFF)
                    value = string[position];
                else
                    value = string[position] + string[position + 1]
            }
            this.stringIteratorFieldIndex = position + value.length;
        }
    }
    return {
        value, done
    }
}

Object.defineProperty(String.prototype, Symbol.iterator, {
    get: function () {
        return function () {
            return new ___StringIterator(this);
        }
    }
})