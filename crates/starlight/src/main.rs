use starlight::{heap::snapshot::Snapshot, vm::Runtime, Platform};

fn main() {
    Platform::initialize();
    let mut rt = Runtime::new(false, None);
    let snapshot = Snapshot::take(&mut rt);

    std::fs::write("snapshot.out", &snapshot.buffer).expect("failed to write snapshot");
    drop(rt);
}
