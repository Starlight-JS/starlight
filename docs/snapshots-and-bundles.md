# Snapshots
Starlight provides API for creating snapshots of runtime heap state and deserializing later. These snapshots could be used to reduce program startup time. 

# Bundles
Bundles is just snapshots plus some small portions of C code to compile snapshots into binaries. `starlight-bundle` is used for compiling JS files to bundle. (***NOTE starlight-bundle works only on Linux for now! Other platforms require you to manually link bundle and use --output-c option***  )

## Serializing/Deserializing snapshot at runtime

```rust

use deserializer::*;
use starlight::vm::*;
use starlight::gc::*;
use snapshot::*;

fn main() {
     let mut rt = Runtime::new(RuntimeParams::default(), GcParams::default(), None);
    /* ... */

    let snapshot = Snapshot::take(false,&mut rt, |_,_| {
        /* you can serialize your own data there */
    });

    let mut rt = Deserializer::deserialize(
        false,
        &snapshot.buffer,
        RuntimeParams::default(),
        default_heap(GcParams::default()),
        None,
        |_,_| {
            /* you can deserialize your own data there */
        },
    );

}
```