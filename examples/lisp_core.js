export default function (global_env) {
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
    global_env['cdr'] = (x) => {
        let new_arr = []
        for (let i = 1; i < x.length; i++) {
            new_arr[i - 1] = x[i];
        }
        return new_arr;
    }
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
}