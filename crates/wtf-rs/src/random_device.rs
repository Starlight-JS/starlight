#[cfg(unix)]
use errno::errno;
#[cfg(unix)]
use libc::{EAGAIN, EINTR};

pub struct RandomDevice {
    #[cfg(not(any(target_os = "macos", target_os = "fuchsia", target_os = "windows")))]
    fd: i32,
}

impl Default for RandomDevice {
    fn default() -> Self {
        Self::new()
    }
}
impl RandomDevice {
    #[cfg(not(any(target_os = "macos", target_os = "fuchsia", target_os = "windows")))]
    pub fn new() -> Self {
        unsafe {
            let mut ret;
            while {
                static DEV_URANDOM: &[u8] = b"/dev/urandom\0";
                ret = libc::open(DEV_URANDOM.as_ptr() as *const i8, libc::O_RDONLY, 0);

                ret == -1 && errno().0 == EINTR
            } {}
            if ret < 0 {
                panic!("unable to open urandom");
            }
            Self { fd: ret }
        }
    }

    #[cfg(any(target_os = "macos", target_os = "fuchsia", target_os = "windows"))]
    pub fn new() -> Self {
        Self {}
    }
    unsafe fn internal_ranom_values(&self, buffer: *mut u8, length: usize) {
        #[cfg(any(target_os = "macos", target_os = "ios"))]
        {
            extern "C" {
                fn CCRandomGenerateBytes(_: *mut u8, _: usize) -> i32;
            }
            assert_eq!(CCRandomGenerateBytes(buffer, length), 0);
        }
        #[cfg(target_os = "fuchsia")]
        {
            extern "C" {
                fn zx_cprng_draw(_: *mut u8, _: usize);
            }
            zx_cprng_draw(buffer, length);
        }
        #[cfg(not(any(target_os = "macos", target_os = "fuchsia", target_os = "windows")))]
        {
            let mut amount_read = 0;
            while amount_read < length {
                let current_read = libc::read(
                    self.fd,
                    buffer.add(amount_read).cast(),
                    length - amount_read,
                );
                if current_read == -1 {
                    if !(errno().0 == EAGAIN || errno().0 == EINTR) {
                        panic!("Unable to read from urandom");
                    }
                } else {
                    amount_read += current_read as usize;
                }
            }
        }
        #[cfg(windows)]
        {
            use winapi::um::wincrypt::*;
            let mut prov: HCRYPTPROV = 0;
            CryptAcquireContextA(
                &mut prov,
                0 as *const _,
                MS_DEF_PROV.as_bytes().as_ptr().cast(),
                PROV_RSA_FULL,
                CRYPT_VERIFYCONTEXT,
            );
            CryptGenRandom(prov, length as _, buffer.cast());
            CryptReleaseContext(prov, 0);
        }
    }
    #[allow(clippy::clippy::not_unsafe_ptr_arg_deref)]
    pub fn cryptographically_random_values(&self, buffer: *mut u8, length: usize) {
        unsafe {
            self.internal_ranom_values(buffer, length);
        }
    }
}

#[cfg(not(any(target_os = "macos", target_os = "fuchsia", target_os = "windows")))]
impl Drop for RandomDevice {
    fn drop(&mut self) {
        unsafe {
            libc::close(self.fd);
        }
    }
}

pub fn cryptographically_random_number_from_os(buffer: &mut [u8]) {
    static DEVICE: once_cell::sync::Lazy<RandomDevice> =
        once_cell::sync::Lazy::new(RandomDevice::new);

    DEVICE.cryptographically_random_values(buffer.as_mut_ptr(), buffer.len());
}
