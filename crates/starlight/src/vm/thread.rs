/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */
use wtf_rs::stack_bounds::StackBounds;

pub struct Thread {
    pub bounds: StackBounds,
}

thread_local! {
    pub static THREAD: Thread = {
        let bounds = StackBounds::current_thread_stack_bounds();
        Thread {
            bounds
        }
    }
}

impl Thread {
    #[cfg(target_arch = "x86_64")]
    pub fn capture_registers() -> [usize; 16] {
        let mut buf = std::mem::MaybeUninit::uninit();
        let buf_ptr = buf.as_mut_ptr();
        unsafe {
            llvm_asm!(
                "
                mov [$0+0], rsp
                mov [$0+8], rax
                mov [$0+16], rbx
                mov [$0+24], rcx
                mov [$0+32], rdx
                mov [$0+40], rbp
                mov [$0+48], rsi
                mov [$0+56], rdi
                mov [$0+64], r8
                mov [$0+72], r9
                mov [$0+80], r10
                mov [$0+88], r11
                mov [$0+96], r12
                mov [$0+104], r13
                mov [$0+112], r14
                mov [$0+120], r15
                

                " :: "r"(buf_ptr) :: "intel"
            );
        }
        unsafe { buf.assume_init() }
    }

    #[cfg(target_arch = "x86")]
    pub fn capture_registers() -> [usize; 0] {
        /*
            on X86 we assume that compiler saved all registers on stack since it does not have that much registers
            available for use.
        */
        []
    }
    #[cfg(target_arch = "arm")]
    pub fn capture_registers() -> [usize; 0] {
        /* TODO */
        []
    }
    #[cfg(target_arch = "aarch64")]
    pub fn capture_registers() -> [usize; 30] {
        let x0;
        let x1;
        let x2;
        let x3;
        let x4;
        let x5;
        let x6;
        let x7;
        let x8;
        let x9;
        let x10;
        let x11;
        let x12;
        let x13;
        let x14;
        let x15;
        let x16;
        let x17;
        let x18;
        let x19;
        let x20;
        let x21;
        let x22;
        let x23;
        let x24;
        let x25;
        let x26;
        let x27;
        let x28;
        let x29;
        unsafe {
            llvm_asm!("mov $0, x0" : "=r"(x0));
            llvm_asm!("mov $0, x1" : "=r"(x1));
            llvm_asm!("mov $0, x2" : "=r"(x2));
            llvm_asm!("mov $0, x3" : "=r"(x3));
            llvm_asm!("mov $0, x4" : "=r"(x4));
            llvm_asm!("mov $0, x5" : "=r"(x5));
            llvm_asm!("mov $0, x6" : "=r"(x6));
            llvm_asm!("mov $0, x7" : "=r"(x7));
            llvm_asm!("mov $0, x8" : "=r"(x8));
            llvm_asm!("mov $0, x9" : "=r"(x9));
            llvm_asm!("mov $0, x10" : "=r"(x10));
            llvm_asm!("mov $0, x11" : "=r"(x11));
            llvm_asm!("mov $0, x12" : "=r"(x12));
            llvm_asm!("mov $0, x13" : "=r"(x13));
            llvm_asm!("mov $0, x14" : "=r"(x14));
            llvm_asm!("mov $0, x15" : "=r"(x15));
            llvm_asm!("mov $0, x16" : "=r"(x16));
            llvm_asm!("mov $0, x17" : "=r"(x17));
            llvm_asm!("mov $0, x18" : "=r"(x18));
            llvm_asm!("mov $0, x19" : "=r"(x19));
            llvm_asm!("mov $0, x20" : "=r"(x20));
            llvm_asm!("mov $0, x21" : "=r"(x21));
            llvm_asm!("mov $0, x22" : "=r"(x22));
            llvm_asm!("mov $0, x23" : "=r"(x23));
            llvm_asm!("mov $0, x24" : "=r"(x24));
            llvm_asm!("mov $0, x25" : "=r"(x25));
            llvm_asm!("mov $0, x26" : "=r"(x26));
            llvm_asm!("mov $0, x27" : "=r"(x27));
            llvm_asm!("mov $0, x28" : "=r"(x28));
            llvm_asm!("mov $0, x29" : "=r"(x29));
            [
                x0, x1, x2, x3, x4, x5, x6, x7, x8, x9, x10, x11, x12, x13, x14, x15, x16, x17,
                x18, x19, x20, x21, x22, x23, x24, x25, x26, x27, x28, x29,
            ]
        }
    }
}
