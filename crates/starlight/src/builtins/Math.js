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