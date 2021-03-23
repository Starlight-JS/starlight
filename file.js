function Node(left, right) {
    this.left = left;
    this.right = right;
}
let nNodes = 0;
function makeTree(depth) {
    nNodes += 1;
    if (depth == 0) {

        return new Node(undefined, undefined);
    }
    let n = depth - 1;

    let n1 = makeTree(n);
    let n2 = makeTree(n);
    return new Node(n1, n2);
}

function populate(depth, node) {

    if (depth <= 0) {
        return;
    }
    depth = depth - 1;

    node.left = new Node(null, null);
    node.right = new Node(null, null);
    populate(depth, node.left);
    populate(depth, node.right);
}

if (!globalThis.console) {
    globalThis.console = {
        log: print
    }
}
let n = makeTree(14);
console.log(nNodes);
