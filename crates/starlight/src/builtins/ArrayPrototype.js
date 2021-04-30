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
    if (length >= 4294967295)
        throw new RangeError("Out of memory for array elements.")

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
    if (length >= 4294967295)
        throw new RangeError("Out of memory for array elements.")

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
    if (length >= 4294967295)
        throw new RangeError("Out of memory for array elements.")

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
    if (length >= 4294967295)
        throw new RangeError("Out of memory for array elements.")

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
    if (length >= 4294967295)
        throw new RangeError("Out of memory for array elements.")

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
function min(left, right) {
    if (left < right) {
        return left;
    }
    return right;
}
function sortMerge(dst, src, srcIndex, srcEnd, width, comparator) {
    "use strict";

    var left = srcIndex;
    var leftEnd = min(left + width, srcEnd);
    var right = leftEnd;
    var rightEnd = min(right + width, srcEnd);

    for (var dstIndex = left; dstIndex < rightEnd; ++dstIndex) {
        if (right < rightEnd) {
            if (left >= leftEnd) {
                dst[dstIndex] = src[right];
                ++right;
                continue;
            }

            var comparisonResult = comparator(src[right], src[left]);
            if (comparisonResult === false || comparisonResult < 0) {

                dst[dstIndex] = src[right];
                ++right;
                continue;
            }

        }

        dst[dstIndex] = src[left];
        ++left;
    }
}


function sortMergeSort(array, comparator) {
    "use strict";

    var valueCount = array.length;
    var buffer = new Array(valueCount);

    var dst = buffer;
    var src = array;
    for (var width = 1; width < valueCount; width = width * 2) {
        for (var srcIndex = 0; srcIndex < valueCount; srcIndex = srcIndex + 2 * width)
            sortMerge(dst, src, srcIndex, valueCount, width, comparator);

        var tmp = src;
        src = dst;
        dst = tmp;
    }

    return src;
}

function sortStringComparator(a, b) {
    "use strict";

    var aString = a.string;
    var bString = b.string;

    if (aString === bString)
        return 0;

    return aString > bString ? 1 : -1;
}

function sortBucketSort(array, dst, bucket, depth) {
    "use strict";

    if (bucket.length < 32 || depth > 32) {
        var sorted = sortMergeSort(bucket, sortStringComparator);
        for (var i = 0; i < sorted.length; ++i) {
            array[dst] = sorted[i].value;
            ++dst;
        }
        return dst;
    }

    var buckets = [];
    for (var i = 0; i < bucket.length; ++i) {
        var entry = bucket[i];
        var string = entry.string;
        if (string.length == depth) {

            array[dst] = entry.value;
            ++dst;
            continue;
        }

        var c = string[depth];
        var cBucket = buckets[c];
        if (cBucket)
            cBucket.push(entry);
        else
            buckets[c] = [entry];
    }

    for (var i = 0; i < buckets.length; ++i) {
        if (!buckets[i])
            continue;
        dst = sortBucketSort(array, dst, buckets[i], depth + 1);
    }

    return dst;
}


function sortCommit(receiver, receiverLength, sorted, undefinedCount) {
    "use strict";

    // Move undefineds and holes to the end of an array. Result is [values..., undefineds..., holes...].

    var sortedLength = sorted.length;

    var i = 0;

    for (; i < sortedLength; ++i)
        receiver[i] = sorted[i];

    for (; i < sortedLength + undefinedCount; ++i)
        receiver[i] = undefined;

    for (; i < receiverLength; ++i)
        delete receiver[i];
}
function sortCompact(receiver, receiverLength, compacted, isStringSort) {
    "use strict";

    var undefinedCount = 0;
    var compactedIndex = 0;

    for (var i = 0; i < receiverLength; ++i) {
        if (i in receiver) {
            var value = receiver[i];
            if (value === undefined)
                ++undefinedCount;
            else {
                if (isStringSort) {
                    compacted[compactedIndex] = {
                        string: toString(value),
                        value: value
                    }
                } else {
                    compacted[compactedIndex] = value;
                }

                ++compactedIndex;
            }
        }
    }

    return undefinedCount;
}

Array.prototype.sort = function (comparator) {
    "use strict";

    var isStringSort = false;
    if (comparator === undefined)
        isStringSort = true;

    var receiver = ___toObject(this, "Array.prototype.sort requires that |this| not be null or undefined");
    var receiverLength = ___toLength(receiver.length);

    // For compatibility with Firefox and Chrome, do nothing observable
    // to the target array if it has 0 or 1 sortable properties.
    if (receiverLength < 2)
        return receiver;

    var compacted = [];
    var sorted = null;
    var undefinedCount = sortCompact(receiver, receiverLength, compacted, isStringSort);
    if (isStringSort) {
        sorted = new Array(compacted.length);
        sortBucketSort(sorted, 0, compacted, 0);
    } else {
        sorted = sortMergeSort(compacted, comparator);
    }
    sortCommit(receiver, receiverLength, sorted, undefinedCount);
    return receiver;
}
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

/*
* TODO: This implementation is wrong. We should implement GetIterator from ECMA262 spec and use it when we see iterator.
*/
Array.from = function from(items, mapFn, thisArg) {
    var mapping;
    if (!mapFn) {
        mapping = false;
    } else {
        if (!___isCallable(mapFn))
            throw new TypeError("mapFn when provided to Array.from should be callable")
        mapping = true;
    }

    var usingIterator = items[Symbol.iterator];
    if (usingIterator !== undefined) {
        var A = ___isConstructor(this) ? new this() : [];

        let iterator = usingIterator;
        let k = 0;
        while (true) {
            if (k >= Number.MAX_SAFE_INTEGER)
                throw new RangeError("max k reached")

            let next = iterator.next();
            if (next.done) {
                A.length = k;
                return A;
            }
            let value = next.value;
            if (mapping) {
                value = thisArg === undefined ? mapFn(value) : mapFn.call(thisArg, value)
            }
            A[k] = value;
            k += 1;
        }
    }

    let arrayLike = ___toObject(items, "Array-like object expected in Array.from");
    let len = ___toLength(arrayLike.length);
    var A = ___isConstructor(this) ? new this() : [];
    let k = 0;
    while (k < len) {
        let kValue = arrayLike[k];
        if (mapping) {
            A[k] = thisArg === undefined ? mapFn(kValue) : mapFn.call(thisArg, kValue)
        } else {
            A[k] = kValue;
        }
        k += 1;
    }
    A.length = k;
    return A;
}



Array.prototype.keys = function () {
    return new ___ArrayIterator(this, "key");
}
let values = function values() {
    return new ___ArrayIterator(this, "value");
}
Array.prototype.values = values;
Array.prototype.entries = function () {
    return new ___ArrayIterator(this, "key+value");
}
Object.defineProperty(Array.prototype, Symbol.iterator, {
    get: function () {
        return values;
    }
})