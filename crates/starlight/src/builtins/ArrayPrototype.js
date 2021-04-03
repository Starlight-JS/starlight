Array.prototype.some = function some(callback, thisArg) {
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


Array.prototype.find = function find(callback, thisArg) {
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

Array.prototype.findIndex = function findIndex(callback, thisArg) {
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

Array.prototype.includes = function includes(searchElement, fromIndex_) {
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

Array.prototype.map = function map(callback, thisArg) {
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

Array.prototype.forEach = function forEach(callback, thisArg) {
    "use strict";
    let array = ___toObject(this, "Array.prototype.forEach requires that |this| not be null or undefined");

    let length = ___toLength(array.length);
    for (let i = 0; i < length; i++) {
        if (i in array) {
            callback.call(thisArg, array[i], i, array);
        }
    }
}

Array.prototype.filter = function filter(callback, thisArg) {
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

Array.prototype.fill = function fill(value, start, end) {
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

function ___moveElements(target, targetOffset, source, sourceLength) {
    for (let i = 0; i < sourceLength; ++i) {
        let value = source[i];
        if (value)
            target[targetOffset + i] = value;
    }
}

function ___append_memory(resultArray, otherArray, startValue) {
    let startIndex = ___toIntegerOrInfinity(startValue);
    ___moveElements(resultArray, startIndex, otherArray, otherArray.length);
}
Array.prototype.sort = function sort(compareFn) {

    return mergeSort(this)
    // Split the array into halves and merge them recursively 
    function mergeSort(arr) {
        if (arr.length === 1) {
            // return once we hit an array with a single item
            return arr
        }
        const middle = ___toIntegerOrInfinity(arr.length / 2) // get the middle item of the array rounded down
        const left = arr.slice(0, middle) // items on the left side
        const right = arr.slice(middle) // items on the right side
        return merge(
            mergeSort(left),
            mergeSort(right)
        )
    }
    // compare the arrays item by item and return the concatenated result
    function merge(left, right) {
        let result = []
        let indexLeft = 0
        let indexRight = 0
        while (indexLeft < left.length && indexRight < right.length) {
            //compareFn ? compareFn =()=> left[indexLeft] < right[indexRight] : compareFn
            let _left = left[indexLeft]
            let _right = right[indexRight]
            if (compareFn)
                compareFn = composeCompareFn(compareFn(left, right))
            compareFn = (l, r) => l < r
            if (compareFn(_left, _right)) {
                result.push(left[indexLeft])
                indexLeft++
            } else {
                result.push(right[indexRight])
                indexRight++
            }
        }
        return result.concat(left.slice(indexLeft)).concat(right.slice(indexRight))
    }
    function composeCompareFn(compareResult) {
        if (compareResult < 0)
            return false
        if (compareResult > 0)
            return true
        if (compareResult == 0)
            return false
    }
}

let flatIntoArray = function flatIntoArray(target, source, sourceLength, targetIndex, depth) {
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
let flatIntoArrayWithCallback = function flatIntoArrayWithCallback(target, source, sourceLength, targetIndex, callback, thisArg) {
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