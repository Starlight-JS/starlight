use crate::bytecode::opcodes::RegisterId;

pub struct RegisterScope {
    registers: Vec<bool>,
    contexts: Vec<Vec<usize>>,
}

impl RegisterScope {
    pub fn new() -> Self {
        Self {
            registers: vec![],
            contexts: vec![vec![]],
        }
    }
    pub fn register(&mut self, reg: usize) -> bool {
        if reg >= self.registers.len() {
            while self.registers.get(reg).is_none() {
                self.registers.push(false);
            }
        }
        self.registers[reg]
    }

    pub fn allocate(&mut self, temp: bool) -> RegisterId {
        for i in 0..self.registers.len() {
            if !self.register(i) {
                self.registers[i] = !temp;
                return i as _;
            }
        }
        let index = self.registers.len();
        let r = self.register(index);
        assert!(!r);
        self.registers[index] = !temp;
        index as _
    }

    pub fn pop_context(&mut self) {
        if let Some(ctx) = self.contexts.pop() {
            for reg in ctx.iter() {
                self.registers[*reg] = false;
            }
        }
    }

    pub fn push_context(&mut self) {
        self.contexts.push(vec![]);
    }

    pub fn protect(&mut self, reg: RegisterId) {
        self.contexts.last_mut().unwrap().push(reg as _);
        self.registers[reg as usize] = true;
    }

    pub fn unprotect(&mut self, reg: RegisterId) {
        let reg = reg as usize;
        self.contexts.last_mut().unwrap().retain(|x| *x != reg);
        self.registers[reg as usize] = false;
    }

    pub fn register_window(&mut self, argc: usize) -> RegisterId {
        let mut start = None;
        let mut found = 0;
        for reg in 0..self.registers.len() {
            if self.registers[reg] == false {
                if let Some(start) = start {
                    found += 1;
                    if found >= argc {
                        for i in start..start + argc {
                            self.registers[i] = true;
                        }
                        return start as _; // start of register window
                    }
                    continue;
                }
                start = Some(reg);
            } else {
                found = 0;
                start = None;
            }
        }

        let start = self.registers.len();
        for i in 0..argc {
            self.register(start + i); // allocate register
            self.registers[start + i] = true;
        }
        start as _
    }

    pub fn unprotect_window(&mut self, argv: RegisterId, argc: usize) {
        for i in 0..argc {
            self.registers[argv as usize + i] = false;
        }
    }
}
