function ___toIntegerOrInfinity(target) {
    "use strict";
    let number_value = +target;
    if (number_value !== number_value || !number_value) {
        return 0;
    }
    return ___trunc(number_value);
}

function ___toLength(target) {
    "use strict";
    let length = ___toIntegerOrInfinity(target);

    return +length;
}

