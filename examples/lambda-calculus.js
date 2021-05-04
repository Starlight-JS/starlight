/* Copyright Freddy A Cubas "Superstar64" */
// λ-calculus. 



var isAlpha = c => c.toUpperCase() != c.toLowerCase(); // ugly hack

var leftAssociative = f => (...xs) => xs.reduce(f);

var empty = () => code => null;
// infinite backtracing
var or = leftAssociative((left, right) => () => code => left()(code) || right()(code));

var pure = item => () => code => ({ just: [item, code] });
var map = (f, ...x) => apply(pure(f), ...x);
var apply = leftAssociative((f, x) => () => code => {
    var monadF = f()(code);
    if (monadF) {
        var monadX = x()(monadF.just[1]);
        if (monadX) {
            return { just: [monadF.just[0](monadX.just[0]), monadX.just[1]] };
        }
    }
    return null;
});

var cons = x => xs => [x].concat(xs);
var many = x => () => or(map(cons, x, many(x)), pure([]))();
var some = x => map(cons, x, many(x));

var satify = check => () => code => {
    if (code.length > 0 && check(code[0])) {
        return { just: [code[0], code.slice(1)] };
    } else {
        return null;
    }
};
var letter = satify(isAlpha);
var literal = x => satify(a => a == x);
var space = many(literal(' '));
var identifier = map(x => x.reduce((a, b) => a + b, ''), some(letter));

var term = () => map(x => x.reduce((f, x) => env => f(env)(x(env))), some(termCore))();
var lambda = map(_ => _ => name => _ => _ => _ => e => env => x => e(augment(env, name, x)),
    literal('|'),
    space,
    identifier,
    space,
    literal('|'),
    space,
    term
);
var variable = map(x => _ => env => env[x], identifier, space);
var parens = map(_ => _ => e => _ => _ => _ => e,
    literal('('),
    space,
    term,
    space,
    literal(')'),
    space
);
var termCore = or(lambda, variable, parens);

var augment = (env, name, x) => {
    var env2 = Object.create(env);
    env2[name] = x;
    return env2;
};


var prelude = {
    two: 2,
    four: 4,
    neg: x => -x,
    add: x => y => x + y,
    sub: x => y => x - y,
    mul: x => y => x * y,
    div: x => y => x / y,
    sqrt: Math.sqrt
};

var run = string => term()(string).just[0](prelude);
// quadratic formula
var quad = run("|a| |b| |c| div (add (neg b) (sqrt ( sub (mul b b) (mul four (mul a c))))) (mul two a)");

print(quad(2)(4)(-20));
