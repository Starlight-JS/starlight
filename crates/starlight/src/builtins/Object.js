Object.defineProperties = function defineProperties(object, properties) {
    let object_ = object;
    let properties_ = properties;

    Object.keys(properties).forEach(function (property) {
        if (property !== '__proto__') {
            Object.defineProperty(object_, property, properties_[property]);

        }
    });
    return object;
}

