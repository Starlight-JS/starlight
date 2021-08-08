use super::register::Register;
use crate::{
    gc::cell::GcPointer,
    vm::{code_block::CodeBlock, value::JsValue},
};
use std::{mem::size_of, mem::transmute};

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct CallSiteIndex {
    bits: u32,
}

impl CallSiteIndex {
    pub fn from_bits(bits: u32) -> Self {
        Self { bits }
    }
    pub fn bytecode_index(self) -> u32 {
        self.bits as _
    }
    pub fn bits(self) -> u32 {
        self.bits
    }
}

#[derive(Copy, Clone, Debug)]
#[repr(C)]
pub struct CallerFrameAndPC {
    pub caller_frame: *mut CallFrame,
    pub return_pc: *mut u8,
}

impl CallerFrameAndPC {
    pub const SIZE_IN_REGS: usize = 2 * size_of::<usize>() / 8;
}

pub enum CallFrameSlot {
    CodeBlock = CallerFrameAndPC::SIZE_IN_REGS as isize,
    Callee = Self::CodeBlock as isize + 1,
    ArgumentCountIncludingThis = Self::Callee as isize + 1,
    ThisArgument = Self::ArgumentCountIncludingThis as isize + 1,
    FirstArgument = Self::ThisArgument as isize + 1,
}

pub struct CallFrame {}

impl CallFrame {
    pub fn caller_frame_and_pc(&self) -> &'static CallerFrameAndPC {
        unsafe { std::mem::transmute(self) }
    }

    pub fn registers(&self) -> *mut Register {
        unsafe { std::mem::transmute(self) }
    }

    pub fn caller_frame_or_entry_frame(&self) -> *mut Self {
        self.caller_frame_and_pc().caller_frame
    }

    pub fn caller_frame(&self) -> *mut Self {
        self.caller_frame_and_pc().caller_frame
    }
    pub extern "C" fn create(base: *mut u8) -> *mut Self {
        base.cast()
    }

    pub extern "C" fn argument_count(&self) -> usize {
        self.argument_count_including_this() - 1
    }

    pub extern "C" fn argument_count_including_this(&self) -> usize {
        unsafe { (*self.at(CallFrameSlot::ArgumentCountIncludingThis as isize)).payload() as _ }
    }
    pub extern "C" fn argument_offset(argument: i32) -> i32 {
        CallFrameSlot::FirstArgument as i32 + argument
    }
    pub extern "C" fn argument_offset_including_this(argument: i32) -> i32 {
        CallFrameSlot::ThisArgument as i32 + argument
    }

    // In the following (argument() and setArgument()), the 'argument'
    // parameter is the index of the arguments of the target function of
    // this frame. The index starts at 0 for the first arg, 1 for the
    // second, etc.
    //
    // The arguments (in this case) do not include the 'this' value.
    // arguments(0) will not fetch the 'this' value. To get/set 'this',
    // use thisValue() and setThisValue() below.

    pub fn address_of_arguments_start(&self) -> *mut JsValue {
        unsafe {
            transmute::<_, *mut Register>(self)
                .offset(Self::argument_offset(0) as _)
                .cast()
        }
    }

    pub fn argument(&self, argument: usize) -> JsValue {
        if argument >= self.argument_count() {
            return JsValue::UNDEFINED;
        }
        unsafe { self.get_argument_unsafe(argument) }
    }

    pub unsafe fn get_argument_unsafe(&self, arg_index: usize) -> JsValue {
        // User beware! This method does not verify that there is a valid
        // argument at the specified argIndex. This is used for debugging
        // and verification code only. The caller is expected to know what
        // he/she is doing when calling this method.
        unsafe {
            transmute::<_, *mut Register>(self)
                .offset(Self::argument_offset(arg_index as _) as _)
                .read()
                .js_val()
        }
    }

    pub fn this_argument_offset() -> i32 {
        Self::argument_offset_including_this(0)
    }
    pub fn this_value(&self) -> JsValue {
        unsafe {
            transmute::<_, *mut Register>(self)
                .offset(Self::this_argument_offset() as _)
                .read()
                .js_val()
        }
    }

    pub fn set_this_value(&mut self, value: JsValue) {
        unsafe {
            transmute::<_, *mut Register>(self)
                .offset(Self::this_argument_offset() as _)
                .write(Register::new(value))
        }
    }

    pub fn unchecked_argument(&self, argument: usize) -> JsValue {
        assert!(argument < self.argument_count());
        unsafe { self.get_argument_unsafe(argument) }
    }
    pub fn set_argument(&mut self, argument: usize, value: JsValue) {
        unsafe {
            transmute::<_, *mut Register>(self)
                .offset(Self::argument_offset(argument as _) as _)
                .write(Register::new(value))
        }
    }
    pub fn new_target(&self) -> JsValue {
        self.this_value()
    }

    pub fn offset_for(argument_count_including_this: usize) -> i32 {
        CallFrameSlot::ThisArgument as i32 + argument_count_including_this as i32 - 1
    }

    pub fn set_argument_count_including_this(&mut self, count: i32) {
        unsafe {
            *(*transmute::<_, *mut Register>(self)
                .offset(CallFrameSlot::ArgumentCountIncludingThis as _))
            .payload_mut() = count;
        }
    }
    pub fn at(&self, ix: isize) -> *mut Register {
        unsafe { self.registers().offset(ix) }
    }

    pub fn call_site_as_raw_bits(&self) -> u32 {
        unsafe {
            self.at(CallFrameSlot::ArgumentCountIncludingThis as _)
                .read()
                .tag() as _
        }
    }

    pub fn call_site_index(&self) -> CallSiteIndex {
        CallSiteIndex::from_bits(self.call_site_as_raw_bits())
    }
    pub fn current_vpc(&self) -> *mut u8 {
        &mut self.code_block().unwrap().code[self.call_site_index().bytecode_index() as usize]
    }

    pub fn set_current_vpc(&mut self, vpc: *mut u8) {
        let start = &self.code_block().unwrap().code[0] as *const u8 as *mut u8;
        let index = vpc as usize - start as usize;
        unsafe {
            *(*self.at(CallFrameSlot::ArgumentCountIncludingThis as _)).tag_mut() =
                index as u32 as i32;
        }
    }

    pub fn bytecode_index(&self) -> usize {
        self.call_site_index().bytecode_index() as usize
    }

    pub fn code_block(&self) -> Option<GcPointer<CodeBlock>> {
        unsafe { self.at(0).read().code_block() }
    }

    fn top_of_frame_internal(&self) -> *mut Register {
        unsafe {
            let code_block = self.code_block().unwrap();
            self.registers()
                .offset(code_block.stack_pointer_offset() as _)
        }
    }
    pub fn top_of_frame(&self) -> *mut Register {
        if self.code_block().is_none() {
            return self.registers();
        }
        self.top_of_frame_internal()
    }
}
