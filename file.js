function tokenize(s) {
    return s.replace(/\(/g, ' ( ').replace(/\)/g, ' ) ').split(' ').filter((val) => val.length);
}

let float_re = /^[-+]?[0-9]*\.?[0-9]+([eE][-+]?[0-9]+)?$/;
function atom(token) {
    if (float_re.exec(token) !== null)
        return parseFloat(token);
    return token + "";
}


function readFrom(tokens) {
    if (tokens.length === 0)
        throw "Unexpected EOF"

    let token = tokens.shift();
    if (token === '(') {
        let L = [];
        while (tokens[0] !== ')')
            L.push(readFrom(tokens));
        tokens.shift();
        return L;
    }
    else if (token == ')')
        throw "unexpected";
    else
        return atom(token)
}


/*
* Constructs new variable environment.
*/
function Env(params, args, outer) {
    for (let i = 0; i < params.length; i++) {
        this[params[i]] = args[i];
    }
    this.outer = outer;
}

Env.prototype.get = function envGet(name) {
    if (name in this) {
        return this;
    }
    if (this.outer !== undefined) {
        return this.outer.get(name);
    }
    throw "Variable '" + name + "' not found";
}
let global_env = new Env([], []);

function eval(x, env) {
    if (!env)
        env = global_env;

    if (typeof x === "string") {
        return env.get(x)[x];
    } else if (!Array.isArray(x))
        return x;
    else if (x[0] === 'quote')
        return x[1];
    else if (x[0] === 'if') {
        let test = x[1];
        let conseq = x[2];
        let alt = x[3];
        return eval(eval(test, env) ? conseq : alt, env);
    } else if (x[0] === 'set!') {
        let var_ = x[1];
        let exp = x[2];
        env.get(var_)[var_] = eval(exp, env);
    } else if (x[0] === 'define') {
        let var_ = x[1];
        let exp = x[2];
        env[var_] = eval(exp, env);
    } else if (x[0] === 'lambda') {
        let vars = x[1];
        let exp = x[2];
        return (...args) => {
            return eval(exp, new Env(vars, args, env));
        }
    } else if (x[0] === 'begin') {
        let val;
        for (let i = 1; i < x.length; i++) {
            val = eval(x[i], env);
        }
        return val;
    } else {
        let exprs = []
        for (let i = 0; i < x.length; i++) {
            exprs[i] = eval(x[i], env)
        }

        proc = exprs.shift();
        return proc(...exprs);
    }
}
function Exit() {

}

global_env['+'] = (...args) => args.reduce((accumulator, val) => accumulator + val, 0)
global_env['-'] = (...args) => args.reduce((accumulator, val) => accumulator - val, 0)
global_env['*'] = (...args) => args.reduce((accumulator, val) => accumulator * val, 1)
global_env['/'] = (...args) => args[0] / args[1]
global_env['exit'] = function exit() { throw new Exit() }
global_env['<'] = (...args) => args[0] < args[1]
global_env['>'] = (...args) => args[0] > args[1]
global_env['<='] = (...args) => args[0] <= args[1]
global_env['>='] = (...args) => args[0] >= args[1]
global_env['eq?'] = (...args) => {
    if (args.length <= 1)
        return false;
    for (let i = 0; i < args.length - 1; i++) {
        if (args[i] != args[i + 1])
            return false;
    }
    return true;
}
global_env['car'] = (x) => x[0]
global_env['cdr'] = (x) => x[1]
global_env['list'] = (...args) => {
    if (args.length === 0)
        return []
    else return args
}
global_env['list?'] = (x) => Array.isArray(x)
global_env['print'] = (...args) => { print(...args) }
global_env['readLine'] = (prompt) => prompt ? readLine(prompt) : readLine()
global_env['trim'] = (arg) => (arg + "").trim()
global_env['null?'] = (item) => item ? false : true
global_env['cons'] = (x, y) => [x, ...y]
global_env['len'] = (x) => x.length
global_env['sym?'] = (x) => typeof x === "string"
global_env['parseNum'] = (x) => parseFloat(x)
global_env['regex'] = (x, flags) => new RegExp("" + x, flags ? flags : "")
global_env['reExec'] = (x, input) => x.exec(input)
global_env['str'] = (...args) => {
    let result = "";
    for (let arg in args) {
        result += arg;
        result += " ";
    }
    return result;
}
function repl() {
    while (true) {
        try {
            let tokens = tokenize(readLine('> ').trim());
            let ast = readFrom(tokens);
            let val = eval(ast);
            if (val !== undefined)
                print(val)
        } catch (e) {
            if (e instanceof Exit) {
                print("exit repl");
                return;
            }
            print('Error: ', e);
        }
    }
}

repl();