function main() {
    function Node(left, right) {
        this.left = left;
        this.right = right;
    }

    function makeTree(depth) {
        var ret = undefined;
        if (depth <= 0) {
            ret = new Node(undefined, undefined);
        } else {
            let n1 = makeTree(depth - 1);
            let n2 = makeTree(depth - 1);
            ret = new Node(n1, n2)
        }
        return ret;
    }

    makeTree(14);
}
main();