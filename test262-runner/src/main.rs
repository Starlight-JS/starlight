use test262_harness::Harness;

fn main() {
    let test262_path = "test262";
    let harness = Harness::new(test262_path).expect("failed to initialize harness");
}
