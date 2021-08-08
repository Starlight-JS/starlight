macro_rules! opcode_list {
    ($f: ident) => {
        $f! {
            enter {},
            create_direct_arguments {
                args: {
                    dst: VirtualRegister
                }
            },
            create_scoped_arguments {
                args: {
                    dst: VirtualRegister
                }
            },
            create_cloned_arguments {
                args: {
                    dst: VirtualRegister
                }
            },
            create_this {
                args: {
                    dst: VirtualRegister,
                    callee: VirtualRegister,
                    inline_capacity: u16,
                },
                metadata: {
                    cached_callee: GcPointer<dyn GcCell>
                }
            },
            create_promise {
                args: {
                    dst: VirtualRegister,
                    callee: VirtualRegister,
                    is_internal: bool
                },
                metadata: {
                    cached_callee: GcPointer<dyn GcCell>
                }
            },
            new_promise {
                args: {
                    dst: VirtualRegister,
                    is_internal: bool
                }
            },
            new_generator {
                args: {
                    dst: VirtualRegister
                }
            },
            create_generator {
                args: {
                    dst: VirtualRegister,
                    callee: VirtualRegister,
                },
                metadata: {
                    cached_callee: GcPointer<dyn GcCell>
                }
            },
            get_argument {
                args: {
                    dst: VirtualRegister,
                    index: i16
                },
                metadata: {
                    profile: ValueProfile
                }
            },
            argument_count {
                args: {
                    dst: VirtualRegister
                }
            },
            to_this {
                args: {
                    src_dst: VirtualRegister,
                    ecma_mode: bool
                }
            },
            check_tdz {
                args: {
                    target: VirtualRegister
                }
            },
            new_object {
                args: {
                    dst: VirtualRegister,
                    inline_capacity: u16
                }
            },
            new_array {
                args: {
                    dst: VirtualRegister,
                    argv: VirtualRegister,
                    argc: u32
                }
            },

        }
    };
}
