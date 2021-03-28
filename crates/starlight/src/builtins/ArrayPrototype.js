Array.prototype.some = function (callback, thisArg) {
    "use strict";
    ___toObject(this, "Array.prototype.some requires that |this| not be null or undefined");
    let length = this.length;

    for (let i = 0; i < length; i += 1) {
        if (!(i in this)) {
            continue;
        }

        if (callback.call(thisArg, this[i], i, this)) {
            return true;
        }
    }
    return false;
}


Array.prototype.find = function (callback, thisArg) {
    "use strict";
    ___toObject(this, "Array.prototype.find requires that |this| not be null or undefined");
    let length = this.length;

    for (let i = 0; i < length; i += 1) {
        let kValue = this[i];
        if (callback.call(thisArg, kValue, i, this)) {
            return kValue;
        }
    }
    return undefined;
}

Array.prototype.findIndex = function (callback, thisArg) {
    "use strict";
    ___toObject(this, "Array.prototype.fromIndex requires that |this| not be null or undefined");
    let length = ___toLength(this.length);

    for (let i = 0; i < length; i += 1) {
        let kValue = this[i];
        if (callback.call(thisArg, kValue, i, this)) {
            return i;
        }
    }
    return undefined;
}

Array.prototype.includes = function (searchElement, fromIndex_) {
    "use strict";
    ___toObject(this, "Array.prototype.includes requires that |this| not be null or undefined");
    let length = ___toLength(this.length);
    if (length === 0) {
        return false;
    }

    let fromIndex = 0;
    let from = fromIndex_;
    if (from !== undefined) {
        fromIndex = ___toIntegerOrInfinity(from);
    }

    let index;
    if (fromIndex >= 0) {
        index = fromIndex;
    } else {
        index = length + fromIndex;
    }

    if (index < 0) {
        index = 0;
    }

    let currentElement;
    for (; index < length; index += 1) {
        currentElement = this[index];
        // Use SameValueZero comparison, rather than just StrictEquals.
        if (searchElement === currentElement || (searchElement != searchElement && currentElement !== currentElement)) {
            return true;
        }
    }

    return false;
}

Array.prototype.map = function (callback, thisArg) {
    "use strict";
    let array = ___toObject(this, "Array.prototype.map requires that |this| not be null or undefined");

    let length = array.length;

    let result = new Array(length);

    for (var i = 0; i < length; i += 1) {
        if (!(i in array)) {
            continue;
        }

        let mappedValue = callback.call(thisArg, array[i], i, array);
        result[i] = mappedValue;
    }
    return result;

}