var global = this;

function f() {
    return gNonStrict();
};
(function () {
    "use strict";
    f.bind(global)();
})();


function gNonStrict() {
    return gNonStrict.caller;
}