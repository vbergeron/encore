pub mod funcs {
    #[allow(dead_code)]
    pub const ADD: usize = 0;
    #[allow(dead_code)]
    pub const EQB: usize = 1;
    #[allow(dead_code)]
    pub const LEB: usize = 2;
    #[allow(dead_code)]
    pub const SHIFT: usize = 3;
    #[allow(dead_code)]
    pub const SUBST: usize = 4;
    #[allow(dead_code)]
    pub const BETA: usize = 5;
    #[allow(dead_code)]
    pub const WHNF: usize = 6;
    #[allow(dead_code)]
    pub const NF: usize = 7;
    #[allow(dead_code)]
    pub const CHURCH: usize = 8;
    #[allow(dead_code)]
    pub const CHURCH_ADD: usize = 9;
    #[allow(dead_code)]
    pub const CHURCH_MUL: usize = 10;
    #[allow(dead_code)]
    pub const READ_CHURCH: usize = 11;
    #[allow(dead_code)]
    pub const TEST_ADD: usize = 12;
    #[allow(dead_code)]
    pub const TEST_MUL: usize = 13;
}

pub mod ctors {
    #[allow(dead_code)]
    pub const NONE: u8 = 5;
    #[allow(dead_code)]
    pub const TRUE: u8 = 1;
    #[allow(dead_code)]
    pub const VAR: u8 = 2;
    #[allow(dead_code)]
    pub const SOME: u8 = 6;
    #[allow(dead_code)]
    pub const APP: u8 = 4;
    #[allow(dead_code)]
    pub const FALSE: u8 = 0;
    #[allow(dead_code)]
    pub const ABS: u8 = 3;
}
