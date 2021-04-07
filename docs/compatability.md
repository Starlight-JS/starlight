# Compatability

Starlight is not compatible with entire ECMAScript standard. Right now it works with ES.51 and ES6, some parts of future standards is also supported. 


# Supported
- Arrow functions
- Array spread
- Call spread

# W.I.P
- `let` and `const`

    Both `let` and `const` work but not correctly when captured in functions.

- `for ..of`,`for ..in`


    `for ..of` already works and `for ..in` loop requires iterators to be implemented.

- Destructive assignments
- Object spread


# Excluded from support
- Realms
- `with` statement
- `eval`, note that `new Function()` will be supported.
- unsafe cases of `finally` (i.e `try { return 42; } finally { return 0; }` <- this code will return `42`)
- And a lot of other features...

# Miscellaneous Incompatibilities#

- `Function.prototype.toString` cannot show source code. Functions is compiled to bytecode and source code is not stored at runtime.
- `arguments` do not have `toString` method. 
