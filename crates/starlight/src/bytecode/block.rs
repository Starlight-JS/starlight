use comet::internal::{finalize_trait::FinalizeTrait, trace_trait::TraceTrait};

use super::{opcodes::OpCode, TypeFeedBack};
use crate::{
    gc::{GcCell, GcPointer},
    vm::{array_storage::ArrayStorage, symbol_table::Symbol},
};

pub enum BytecodeBlock {
    Compiled(BytecodeBlockInternal),
}
impl BytecodeBlock {
    pub fn compiled(&self) -> &BytecodeBlockInternal {
        match self {
            Self::Compiled(x) => x,
            _ => unreachable!(),
        }
    }

    pub fn compiled_mut(&mut self) -> &mut BytecodeBlockInternal {
        match self {
            Self::Compiled(x) => x,
            _ => unreachable!(),
        }
    }
}
impl GcCell for BytecodeBlock {}
impl TraceTrait for BytecodeBlock {
    fn trace(&self, vis: &mut comet::visitor::Visitor) {
        match self {
            Self::Compiled(ref block) => {
                block.trace(vis);
            }
            _ => (),
        }
    }
}

pub struct BytecodeBlockInternal {
    pub(crate) code: Vec<OpCode>,
    pub(crate) literals: GcPointer<ArrayStorage>,
    pub(crate) parent_blocks: Vec<GcPointer<BytecodeBlock>>,
    pub(crate) feedback: Vec<TypeFeedBack>,
    pub(crate) names: Vec<Symbol>,
}

impl TraceTrait for BytecodeBlockInternal {
    fn trace(&self, vis: &mut comet::visitor::Visitor) {
        self.literals.trace(vis);
        self.parent_blocks.trace(vis);
        self.feedback.trace(vis);
    }
}

impl FinalizeTrait<BytecodeBlock> for BytecodeBlockInternal {}
impl FinalizeTrait<BytecodeBlock> for BytecodeBlock {}
