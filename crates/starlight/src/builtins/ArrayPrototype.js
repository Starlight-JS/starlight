Array.prototype.some = function (callback, thisArg) {
    "use strict";
    let array = ___toObject(this, "Array.prototype.some requires that |this| not be null or undefined");
    let length = array.length;

    for (let i = 0; i < length; i += 1) {
        if (!(i in array)) {
            continue;
        }

        if (callback.call(thisArg, array[i], i, array)) {
            return true;
        }
    }
    return false;
}


Array.prototype.find = function (callback, thisArg) {
    "use strict";
    let array = ___toObject(this, "Array.prototype.find requires that |this| not be null or undefined");
    let length = array.length;

    for (let i = 0; i < length; i += 1) {
        let kValue = array[i];
        if (callback.call(thisArg, kValue, i, array)) {
            return kValue;
        }
    }
    return undefined;
}

Array.prototype.findIndex = function (callback, thisArg) {
    "use strict";
    let array = ___toObject(this, "Array.prototype.fromIndex requires that |this| not be null or undefined");
    let length = ___toLength(array.length);

    for (let i = 0; i < length; i += 1) {
        let kValue = array[i];
        if (callback.call(thisArg, kValue, i, array)) {
            return i;
        }
    }
    return -1;
}

Array.prototype.includes = function (searchElement, fromIndex_) {
    "use strict";
    let array = ___toObject(this, "Array.prototype.includes requires that |this| not be null or undefined");
    let length = ___toLength(array.length);
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
        currentElement = array[index];
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

    for (let i = 0; i < length; i += 1) {
        if (!(i in array)) {
            continue;
        }

        let mappedValue = callback.call(thisArg, array[i], i, array);
        result[i] = mappedValue;
    }
    return result;
}

Array.prototype.forEach = function (callback, thisArg) {
    "use strict";
    let array = ___toObject(this, "Array.prototype.forEach requires that |this| not be null or undefined");

    let length = ___toLength(array.length);
    for (let i = 0; i < length; i++) {
        if (i in array) {
            callback.call(thisArg, array[i], i, array);
        }
    }
}

Array.prototype.filter = function (callback, thisArg) {
    "use strict";
    let array = ___toObject(this, "Array.prototype.filter requires that |this| not be null or undefined");
    let length = ___toLength(array.length);

    let result = [];
    let nextIndex = 0;
    for (let i = 0; i < length; i++) {
        if (!(i in array)) {
            continue;
        }

        let current = array[i];
        if (callback.call(thisArg, current, i, array)) {
            result[nextIndex] = current;
            ++nextIndex;
        }
    }
    return result;
}