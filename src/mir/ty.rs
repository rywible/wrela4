#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AccessMode {
    Read,
    Mut,
    Own,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScalarType {
    Bool,
    I64,
    U32,
    U64,
    String,
    None,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MirType {
    Scalar(ScalarType),
    Data(String),
    LayoutData(String),
    Class(String),
    UniqueClass(String),
    Interface(String),
    Error(String),
    Image(String),
    HostImage(String),
    Capability {
        class_name: String,
        path: String,
    },
    Table {
        item: String,
        rows: u64,
    },
    Column {
        item: String,
        table: String,
        rows: u64,
    },
    Mask {
        table: String,
        rows: u64,
    },
    RowToken {
        table: String,
    },
    StateToken(StateTokenKind),
    CapacityToken {
        owner: String,
    },
    Never,
    Unknown,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StateTokenKind {
    Table(String),
    Mmio(String),
    Atomic(String),
    Sync(String),
}
