pub struct Heap {}

pub struct GcPointerBase {
    #[cfg(not(feature = "compressed-rc"))]
    strong_count: u32,
    #[cfg(not(feature = "compressed-rc"))]
    weak_count: u32,
}
