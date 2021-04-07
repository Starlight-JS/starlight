Array.prototype.some = function some(callback, thisArg) {
    "use strict";
    var array = ___toObject(this, "Array.prototype.some requires that |this| not be null or undefined");
    var length = array.length;

    for (var i = 0; i < length; i += 1) {
        if (!(i in array)) {
            continue;
        }

        if (callback.call(thisArg, array[i], i, array)) {
            return true;
        }
    }
    return false;
}


Array.prototype.find = function find(callback, thisArg) {
    "use strict";
    var array = ___toObject(this, "Array.prototype.find requires that |this| not be null or undefined");
    var length = array.length;

    for (var i = 0; i < length; i += 1) {
        var kValue = array[i];
        if (callback.call(thisArg, kValue, i, array)) {
            return kValue;
        }
    }
    return undefined;
}

Array.prototype.findIndex = function findIndex(callback, thisArg) {
    "use strict";
    var array = ___toObject(this, "Array.prototype.fromIndex requires that |this| not be null or undefined");
    var length = ___toLength(array.length);

    for (var i = 0; i < length; i += 1) {
        var kValue = array[i];
        if (callback.call(thisArg, kValue, i, array)) {
            return i;
        }
    }
    return -1;
}

Array.prototype.includes = function includes(searchElement, fromIndex_) {
    "use strict";
    var array = ___toObject(this, "Array.prototype.includes requires that |this| not be null or undefined");
    var length = ___toLength(array.length);
    if (length === 0) {
        return false;
    }

    var fromIndex = 0;
    var from = fromIndex_;
    if (from !== undefined) {
        fromIndex = ___toIntegerOrInfinity(from);
    }

    var index;
    if (fromIndex >= 0) {
        index = fromIndex;
    } else {
        index = length + fromIndex;
    }

    if (index < 0) {
        index = 0;
    }

    var currentElement;
    for (; index < length; index += 1) {
        currentElement = array[index];
        // Use SameValueZero comparison, rather than just StrictEquals.
        if (searchElement === currentElement || (searchElement != searchElement && currentElement !== currentElement)) {
            return true;
        }
    }

    return false;
}

Array.prototype.map = function map(callback, thisArg) {
    "use strict";
    var array = ___toObject(this, "Array.prototype.map requires that |this| not be null or undefined");

    var length = array.length;

    var result = new Array(length);
    for (var i = 0; i < length; i += 1) {

        if (!(i in array)) {
            continue;
        }

        var mappedValue = callback.call(thisArg, array[i], i, array);
        result[i] = mappedValue;
    }
    return result;
}

Array.prototype.forEach = function forEach(callback, thisArg) {
    "use strict";
    var array = ___toObject(this, "Array.prototype.forEach requires that |this| not be null or undefined");

    var length = ___toLength(array.length);
    for (var i = 0; i < length; i++) {
        if (i in array) {
            callback.call(thisArg, array[i], i, array);
        }
    }
}

Array.prototype.filter = function filter(callback, thisArg) {
    "use strict";
    var array = ___toObject(this, "Array.prototype.filter requires that |this| not be null or undefined");
    var length = ___toLength(array.length);

    var result = [];
    var nextIndex = 0;
    for (var i = 0; i < length; i++) {
        if (!(i in array)) {
            continue;
        }

        var current = array[i];
        if (callback.call(thisArg, current, i, array)) {
            result[nextIndex] = current;
            ++nextIndex;
        }
    }
    return result;
}

Array.prototype.fill = function fill(value, start, end) {
    "use strict";
    var array = ___toObject(this, "Array.prototype.fill requires that |this| not be null or undefined");
    var length = ___toLength(array.length);

    var relativeStart = ___toIntegerOrInfinity(start);
    var k = 0;
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

    var relativeEnd = length;

    if (end !== undefined) {
        relativeEnd = ___toIntegerOrInfinity(end);
    }
    var final = 0;
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
    var undefinedCount = 0;
    var compactedIndex = 0;
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

function ___moveElements(target, targetOffset, source, sourceLength) {
    for (var i = 0; i < sourceLength; ++i) {
        var value = source[i];
        if (value)
            target[targetOffset + i] = value;
    }
}

function ___append_memory(resultArray, otherArray, startValue) {
    var startIndex = ___toIntegerOrInfinity(startValue);
    ___moveElements(resultArray, startIndex, otherArray, otherArray.length);
}

Array.prototype.sort = function (cmp) {
    "use strict";
    var arr = ___toObject(this, "Array.prototype.sort requires that |this| not be null or undefined");
    var fn = (l, r) => l < r || cmp;

    var length = arr.length;
    var mutableArray = new Array(length);
    var buffer = new Array(length);

    function destructiveSort(offset, length) {
        // sorting zero or one elements is the degenerate case.
        if (length > 1) {
            var halfLength = ___toLength(length / 2);

            // sort the first and second haves in place
            destructiveSort(offset, halfLength);
            destructiveSort(offset + halfLength, length - halfLength);

            // now merge the two sorted sublists into the buffer
            var z1 = offset + halfLength,
                z2 = offset + length;

            var i1 = offset,
                i2 = offset + halfLength,
                i3 = offset;

            while (i1 < z1 && i2 < z2) {
                if (fn(mutableArray[i1], mutableArray[i2])) {
                    buffer[i3++] = mutableArray[i1++];
                }
                else buffer[i3++] = mutableArray[i2++];
            }
            while (fn(i1, z1)) buffer[i3++] = mutableArray[i1++];
            while (fn(i2, z2)) buffer[i3++] = mutableArray[i2++];

            // and copy the buffer back to the origial
            for (var i = offset; i < z2; ++i) mutableArray[i] = buffer[i];

        }
    }

    for (var i = 0; i < length; ++i) mutableArray[i] = arr[i];
    destructiveSort(0, length)
    return mutableArray;
};

var flatIntoArray = function flatIntoArray(target, source, sourceLength, targetIndex, depth) {
    "use strict";

    for (var sourceIndex = 0; sourceIndex < sourceLength; ++sourceIndex) {
        if (sourceIndex in source) {
            var element = source[sourceIndex];
            if (depth > 0 && Array.isArray(element))
                targetIndex = flatIntoArray(target, element, ___toLength(element.length), targetIndex, depth - 1);
            else {

                target[targetIndex] = element;
                ++targetIndex;
            }
        }
    }
    return targetIndex;
}
Array.prototype.flat = function (depth) {
    "use strict";

    var array = ___toObject(this, "Array.prototype.flat requires that |this| not be null or undefined");
    var length = ___toLength(array.length);
    var depthNum = 1;

    if (depth !== undefined)
        depthNum = ___toIntegerOrInfinity(depth);

    var result = []

    flatIntoArray(result, array, length, 0, depthNum);
    return result;
}
var flatIntoArrayWithCallback = function flatIntoArrayWithCallback(target, source, sourceLength, targetIndex, callback, thisArg) {
    "use strict";

    for (var sourceIndex = 0; sourceIndex < sourceLength; ++sourceIndex) {
        if (sourceIndex in source) {
            var element = callback.call(thisArg, source[sourceIndex], sourceIndex, source);
            if (Array.isArray(element))
                targetIndex = flatIntoArray(target, element, ___toLength(element.length), targetIndex, 0);
            else {
                target[targetIndex] = element;

                ++targetIndex;
            }
        }
    }
    return target;
}

Array.prototype.flatMap = function flatMap(callback, thisArg) {
    "use strict";

    var array = ___toObject(this, "Array.prototype.flatMap requires that |this| not be null or undefined");
    var length = ___toLength(array.length);

    if (!___isCallable(callback))
        throw new TypeError("Array.prototype.flatMap callback must be a function");


    var result = []

    return flatIntoArrayWithCallback(result, array, length, 0, callback, thisArg);
}

Array.prototype.at = function at(index) {
    "use strict";

    var array = ___toObject(this, "Array.prototype.at requires that |this| not be null or undefined");
    var length = ___toLength(array.length);

    var k = ___toIntegerOrInfinity(index);
    if (k < 0)
        k += length;

    return (k >= 0 && k < length) ? array[k] : undefined;
}