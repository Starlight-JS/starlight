use serde::{Deserialize, Serialize};
use std::io;
use std::path::Path;

/// A view into the heap at a particular point in execution.
#[derive(Deserialize, Serialize, Debug, Clone, PartialEq, Eq)]
pub struct HeapSnapshot {
    /// How many objects are allocated on the heap in total.
    pub object_count: usize,
    /// The total size of the heap in bytes.
    pub total_size: usize,
    /// The objects allocated on the heap and their size.
    ///
    /// TODO(RDambrosio016): change this to a proper type which tells what type it is
    /// and what its name is once `Structure` and `Class` are finalized.
    pub objects: Vec<usize>,
}

impl HeapSnapshot {
    /// Write this snapshot to a file in JSON format.
    pub fn write_to_file(&self, path: impl AsRef<Path>) -> io::Result<()> {
        std::fs::write(path, serde_json::to_string(self).unwrap())
    }
}

#[cfg(test)]
mod tests {
    use crate::runtime::vm::JsVirtualMachine;
    use crate::runtime::{js_string::JsString, options::Options};
    use crate::util::array::GcVec;

    #[test]
    fn heap_snapshot() {
        let mut vm = JsVirtualMachine::create(Options::default());
        let _ = JsString::new(vm, "Hello,World!");
        let _ = GcVec::<i32>::new(vm, 1);
        let snapshot = vm.record_heap_snapshot();
        assert_eq!(snapshot.object_count, 2);
        assert_ne!(snapshot.total_size, 0);
        for obj in snapshot.objects {
            assert_ne!(obj, 0);
        }
    }
}
