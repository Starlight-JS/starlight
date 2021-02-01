use core::{cell::UnsafeCell, mem::MaybeUninit};
use once_cell::sync::Lazy;
use parking_lot::{lock_api::RawMutex, RawMutex as Lock};

use crate::random_device::cryptographically_random_number_from_os;

#[derive(Clone, Copy)]
struct ARC4Stream {
    i: u8,
    j: u8,
    s: [u8; 256],
}

impl ARC4Stream {
    fn new() -> Self {
        let mut slice = [0; 256];
        for n in 0..256 {
            slice[n] = n as u8;
        }
        Self {
            j: 0,
            i: 0,
            s: slice,
        }
    }
}

pub struct ARC4RandomNumberGenerator {
    stream: ARC4Stream,
    count: i32,
    mutex: Lock,
}

impl ARC4RandomNumberGenerator {
    fn new() -> Self {
        Self {
            count: 0,
            mutex: Lock::INIT,
            stream: ARC4Stream::new(),
        }
    }

    fn add_random_data(&mut self, data: &[u8]) {
        self.stream.i = self.stream.i.wrapping_sub(1);
        for n in 0..256 {
            self.stream.i = self.stream.i.wrapping_add(1);
            let si = self.stream.s[self.stream.i as usize];
            self.stream.j = self
                .stream
                .j
                .wrapping_add(si.wrapping_add(data[n as usize % data.len()]));
            self.stream.s[self.stream.i as usize] = self.stream.s[self.stream.j as usize];
            self.stream.s[self.stream.j as usize] = si;
        }
        self.stream.j = self.stream.i;
    }
    fn get_byte(&mut self) -> u8 {
        self.stream.i = self.stream.i.wrapping_add(1);
        let si = self.stream.s[self.stream.i as usize];
        self.stream.j = self.stream.j.wrapping_add(si);
        let sj = self.stream.s[self.stream.j as usize];
        self.stream.s[self.stream.i as usize] = sj;
        self.stream.s[self.stream.j as usize] = si;
        return self.stream.s[(si.wrapping_add(sj) as usize)] & 0xff;
    }

    fn get_word(&mut self) -> u32 {
        let mut val: u32;
        val = (self.get_byte() as u32) << 24;
        val |= (self.get_byte() as u32) << 16;
        val |= (self.get_byte() as u32) << 8;
        val |= self.get_byte() as u32;
        val
    }

    fn stir(&mut self) {
        unsafe {
            let mut randomness: [u8; 128] = MaybeUninit::uninit().assume_init();
            cryptographically_random_number_from_os(&mut randomness);
            self.add_random_data(&randomness);
            // Discard early keystream, as per recommendations in:
            // http://www.wisdom.weizmann.ac.il/~itsik/RC4/Papers/Rc4_ksa.ps
            for _ in 0..256 {
                self.get_byte();
            }
            self.count = 1600000;
        }
    }

    pub fn random_number(&mut self) -> u32 {
        self.mutex.lock();
        self.count -= 4;
        self.stir_if_needed();
        let word = self.get_word();
        unsafe { self.mutex.unlock() };
        word
    }

    pub fn stir_if_needed(&mut self) {
        if self.count <= 0 {
            self.stir()
        }
    }

    pub fn random_values(&mut self, buffer: &mut [u8]) {
        self.mutex.lock();
        self.stir_if_needed();
        let mut length = buffer.len();
        while length != 0 {
            length -= 1;
            self.count -= 1;
            self.stir_if_needed();
            buffer[length] = self.get_byte();
        }
        unsafe { self.mutex.unlock() }
    }
}
unsafe impl Send for ARC4RandomNumberGenerator {}
unsafe impl Sync for ARC4RandomNumberGenerator {}

struct Handle(UnsafeCell<ARC4RandomNumberGenerator>);

unsafe impl Send for Handle {}
unsafe impl Sync for Handle {}
pub fn shared_random_number_generator() -> &'static mut ARC4RandomNumberGenerator {
    static GEN: Lazy<Handle> =
        Lazy::new(|| Handle(UnsafeCell::new(ARC4RandomNumberGenerator::new())));
    unsafe { &mut *(*GEN).0.get() }
}

pub fn cryptographically_random_number() -> u32 {
    shared_random_number_generator().random_number()
}

pub fn cryptographically_random_values(buffer: &mut [u8]) {
    shared_random_number_generator().random_values(buffer)
}
