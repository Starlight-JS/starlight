pub mod block;

pub mod block_set;

pub mod handle;
pub mod heap;

#[cfg(feature = "valgrind-gc")]
pub(crate) mod valgrind;

pub struct FormattedSize {
    size: usize,
}

impl std::fmt::Display for FormattedSize {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let ksize = (self.size as f64) / 1024f64;

        if ksize < 1f64 {
            return write!(f, "{}B", self.size);
        }

        let msize = ksize / 1024f64;

        if msize < 1f64 {
            return write!(f, "{:.1}K", ksize);
        }

        let gsize = msize / 1024f64;

        if gsize < 1f64 {
            write!(f, "{:.1}M", msize)
        } else {
            write!(f, "{:.1}G", gsize)
        }
    }
}

pub fn formatted_size(size: usize) -> FormattedSize {
    FormattedSize { size }
}
