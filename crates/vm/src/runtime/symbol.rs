use lasso::Spur;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub enum Symbol {
    Key(Option<Spur>),
    Indexed(u32),
}

pub const DUMMY_SYMBOL: Symbol = Symbol::Key(None);
