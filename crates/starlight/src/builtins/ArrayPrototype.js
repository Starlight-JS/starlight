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

Array.prototype.fill = function (value, start, end) {
    "use strict";
    let array = ___toObject(this, "Array.prototype.fill requires that |this| not be null or undefined");
    let length = ___toLength(array.length);

    let relativeStart = ___toIntegerOrInfinity(start);
    let k = 0;
    if (relativeStart < 0) {
        k = length + relativeStart;
        if (k < 0) {
            k = 0;
        }
    } else {
        k = relativeStart;
        if (k > length) {
            k = length;
        }
    }

    let relativeEnd = length;

    if (end !== undefined) {
        relativeEnd = ___toIntegerOrInfinity(end);
    }
    let final = 0;
    if (relativeEnd < 0) {
        final = length + relativeEnd;
        if (final < 0)
            final = 0;

    } else {
        final = relativeEnd;
        if (final > length)
            final = length
    }

    for (; k < final; k++)
        array[k] = value;

    return array;
}

function ___sortCompact(receiver, receiverLength, compacted, isStringSort) {
    "use strict";
    let undefinedCount = 0;
    let compactedIndex = 0;
    for (var i = 0; i < receiverLength; ++i) {
        if (i in receiver) {
            var value = receiver[i];
            if (value === undefined)
                ++undefinedCount;
            else {
                compacted[compactedIndex] = isStringSort ? { string: toString(value), value } : value;
                ++compactedIndex;
            }
        }
    }
    return undefinedCount;
}

function ___sortCommit(receiver, receiverLength, sorted, undefinedCount) {

}

Array.prototype.sort = function (comparator) {
    "use strict";

    let isStringSort = false;
    if (comparator === undefined) isStringSort = false;
    else if (!___isCallable(comparator))
        throw new TypeError("Array.prototype.sort requires the comparator argument to be a function or undefined");

    let receiver = ___toObject(this, "Array.prototype.sort requires that |this| not be null or undefined");
    let receiverLength = ___toLength(receiver.length);
    // For compatibility with Firefox and Chrome, do nothing observable
    // to the target array if it has 0 or 1 sortable properties.
    if (receiverLength < 2) {
        return receiver;
    }

    let compacted = [];
    let sorted = null;


}