function Node(left, right) {
    this.left = left;
    this.right = right;
}
let nNodes = 0;
function makeTree(depth) {
    //  nNodes += 1;
    if (depth == 0) {
        return new Node(undefined, undefined);
    }

    let n1 = makeTree(depth - 1);
    let n2 = makeTree(depth - 1);
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

makeTree(14);