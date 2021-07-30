pub mod conventions;
pub mod opcode_list;
pub mod virtual_register;
pub type RegisterIndex = u16;

pub const REGISTER_LIMIT: u16 = u16::MAX;
pub const REGULAR_REGISTER_LIMIT: u16 = REGISTER_LIMIT / 2;
pub const VARIABLE_LIMIT: u16 = REGISTER_LIMIT / 4;
pub const BINDED_NUMERAL_VARIABLE_LIMIT: u16 = REGISTER_LIMIT / 4;
pub type LexicalBlockIndex = u16;

macro_rules! for_each_bytecode_op {
    ($f: ident) => {
        $f! {
            LoadLiteral, 1, 0,
            LoadByName, 1, 0,
            StoreByName, 0, 0,
            InitializeByName, 0, 0,
            LoadByHeapIndex, 1, 0,
            StoreByHeapIndex, 0, 0,
            InitializeByHeapIndex, 0, 0,
            NewOperation, 1, 0,
            NewOperationWithSpreadElement, 1, 0,
            BinaryPlus, 1, 2,
            BinaryMinus, 1, 2,
            BinaryMultiply, 1, 2,
            BinaryDivision, 1, 2,
            BinaryExponentiation, 1, 2,
            BinaryMod, 1, 2,
            BinaryEqual, 1, 2,
            BinaryLessThan, 1, 2,
            BinaryLessThanOrEqual, 1, 2,
            BinaryGreaterThan, 1, 2,
            BinaryGreaterThanOrEqual, 1, 2,
            BinaryNotEqual, 1, 2,
            BinaryStrictEqual, 1, 2,
            BinaryNotStrictEqual, 1, 2,
            BinaryBitwiseAnd, 1, 2,
            BinaryBitwiseOr, 1, 2,
            BinaryBitwiseXor, 1, 2,
            BinaryLeftShift, 1, 2,
            BinarySignedRightShift, 1, 2,
            BinaryUnsignedRightShift, 1, 2,
            BinaryInOperation, 1, 2,
            BinaryInstanceOfOperation, 1, 2,
            BreakpointDisabled, 0, 0,
            BreakpointEnabled, 0, 0,
            CreateObject, 1, 0,
            CreateArray, 1, 0,
            CreateSpreadArrayObject, 1, 0,
            CreateFunction, 1, 0,
            InitializeClass, 0, 0,
            CreateRestElement, 0, 0,
            SuperReference, 1, 0,
            ComplexSetObjectOperation, 0, 2,
            ComplexGetObjectOperation, 1, 2,
            LoadThisBinding, 0, 0,
            ObjectDefineOwnPropertyOperation, 0, 0,
            ObjectDefineOwnPropertyWithNameOperation, 0, 0,
            ArrayDefineOwnPropertyOperation, 0, 0,
            ArrayDefineOwnPropertyBySpreadElementOperation, 0, 0,
            GetObject, 1, 2,
            SetObjectOperation, 0, 2,
            GetObjectPreComputedCase, 1, 1,
            SetObjectPreComputedCase, 0, 1,
            GetGlobalVariable, 1, 1,
            SetGlobalVariable, 0, 1,
            InitializeGlobalVariable, 0, 1,
            Move, 1, 0,
            Increment, 1, 1,
            Decrement, 1, 1,
            ToNumericIncrement, 2, 2,
            ToNumericDecrement, 2, 2,
            ToNumber, 1, 1,
            UnaryMinus, 1, 1,
            UnaryNot, 1, 1,
            UnaryBitwiseNot, 1, 1,
            UnaryTypeof, 1, 1,
            UnaryDelete, 1, 1,
            TemplateOperation, 1, 1,
            Jump, 0, 0,
            JumpComplexCase, 0, 0,
            JumpIfTrue, 0, 0,
            JumpIfUndefinedOrNull, 0, 0,
            JumpIfFalse, 0, 0,
            JumpIfNotFulfilled, 0, 0,
            JumpIfEqual, 0, 0,
            CallFunction, -1, 0,
            CallFunctionWithReceiver, -1, 0,
            GetParameter, 0, 0,
            ReturnFunctionSlowCase, 0, 0,
            TryOperation, 0, 0,
            CloseLexicalEnvironment, 0, 0,
            ThrowOperation, 0, 0,
            ThrowStaticErrorOperation, 0, 0,
            CreateEnumerateObject, 1, 0,
            GetEnumerateKey, 1, 0,
            CheckLastEnumerateKey, 0, 0,
            MarkEnumerateKey, 2, 0,
            IteratorOperation, 0, 0,
            GetMethod, 0, 0,
            LoadRegExp, 1, 0,
            OpenLexicalEnvironment, 0, 0,
            ObjectDefineGetterSetter, 0, 0,
            CallFunctionComplexCase, 0, 0,
            BindingRestElement, 1, 0,
            ExecutionResume, 0, 0,
            ExecutionPause, 0, 0,
            MetaPropertyOperation, 1, 0,
            BlockOperation, 0, 0,
            ReplaceBlockLexicalEnvironmentOperation, 0, 0,
            TaggedTemplateOperation, 0, 0,
            EnsureArgumentsObject, 0, 0,
            ResolveNameAddress, 1, 0,
            StoreByNameWithAddress, 0, 1,
            End, 0, 0,
        }
    };
}

macro_rules! declare_opcode {
    ($($name: ident,$pc: expr,$popc: expr,)*) => {
        #[derive(Copy,Clone,PartialEq,Eq,PartialOrd,Ord,Debug,Hash)]
        pub enum OpCode {
            $(
                $name,
            )*
            KindEnd,
            // special opcode only used in interpreter
            GetObjectOpcodeSlowCaseOpcode,
            SetObjectOpcodeSlowCaseOpcode,
        }
    };
}

for_each_bytecode_op!(declare_opcode);

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Hash)]
pub struct ByteCodeLOC {
    pub index: usize,
    #[cfg(feature = "debug-bc")]
    pub line: usize,
    #[cfg(feature = "debug-bc")]
    pub column: usize,
}

impl ByteCodeLOC {
    pub const fn new(index: usize) -> Self {
        Self {
            index,
            #[cfg(feature = "debug-bc")]
            line: usize::MAX,
            #[cfg(feature = "debug-bc")]
            column: usize::MAX,
        }
    }
}
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Hash)]
#[repr(C)]
pub struct ByteCode {
    pub opcode: OpCode,
    #[cfg(feature = "debug-bc")]
    pub loc: ByteCodeLOC,
}

impl ByteCode {
    #[inline(always)]
    pub const fn new(code: OpCode, loc: ByteCodeLOC) -> Self {
        let _ = loc;
        Self {
            opcode: code,
            #[cfg(feature = "debug-bc")]
            loc,
        }
    }
}
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Hash)]
#[repr(C)]
pub struct LoadLiteral {
    pub op: ByteCode,
    pub dst: RegisterIndex,
    pub literal: u32,
}

impl LoadLiteral {
    pub const fn new(loc: ByteCodeLOC, dst: RegisterIndex, literal: u32) -> Self {
        Self {
            op: ByteCode::new(OpCode::LoadLiteral, loc),
            dst,
            literal,
        }
    }

    pub fn dump(&self) {
        print!("load r{} <- c{}", self.dst, self.literal);
    }
}
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Hash)]
#[repr(u8)]
pub enum MetaPropertyOperationType {
    NewTarget,
    ImportMeta,
}
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Hash)]
#[repr(C)]
pub struct MetaPropertyOperation {
    pub op: ByteCode,
    pub ty: MetaPropertyOperationType,
    pub ri: RegisterIndex,
}

impl MetaPropertyOperation {
    pub const fn new(loc: ByteCodeLOC, ty: MetaPropertyOperationType, ri: RegisterIndex) -> Self {
        Self {
            op: ByteCode::new(OpCode::MetaPropertyOperation, loc),
            ty,
            ri,
        }
    }

    pub fn dump(&self) {
        print!("r{} <- new.target", self.ri)
    }
}
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Hash)]
#[repr(C)]
pub struct LoadByName {
    pub op: ByteCode,
    pub ri: RegisterIndex,
    pub name: u32,
}

impl LoadByName {
    pub const fn new(loc: ByteCodeLOC, ri: RegisterIndex, name: u32) -> Self {
        Self {
            op: ByteCode::new(OpCode::LoadByName, loc),
            ri,
            name,
        }
    }

    pub fn dump(&self) {
        print!("load r{} <- var({})", self.ri, self.name)
    }
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Hash)]
#[repr(C)]
pub struct StoreByName {
    pub op: ByteCode,
    pub ri: RegisterIndex,
    pub name: u32,
}

impl StoreByName {
    pub const fn new(loc: ByteCodeLOC, ri: RegisterIndex, name: u32) -> Self {
        Self {
            op: ByteCode::new(OpCode::StoreByName, loc),
            ri,
            name,
        }
    }

    pub fn dump(&self) {
        print!("store var({}) <- r{}", self.name, self.ri);
    }
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Hash)]
#[repr(C)]
pub struct InitializeByName {
    pub op: ByteCode,
    pub is_lexically_declared_name: bool,
    pub ri: RegisterIndex,
    pub name: u32,
}

impl InitializeByName {
    pub fn new(
        loc: ByteCodeLOC,
        ri: RegisterIndex,
        name: u32,
        is_lexically_declared_name: bool,
    ) -> Self {
        Self {
            op: ByteCode::new(OpCode::InitializeByName, loc),
            is_lexically_declared_name,
            ri,
            name,
        }
    }

    pub fn dump(&self) {
        print!("initialize var({}) <- r{}", self.name, self.ri)
    }
}
