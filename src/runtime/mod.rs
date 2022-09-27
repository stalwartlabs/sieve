use crate::Capability;

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum StringItem {
    Text(Vec<u8>),
    VariableName(String),
    VariableNumber(usize),
    MatchMany(usize),
    MatchOne,
    List(Vec<StringItem>),
}

#[derive(Debug)]
pub(crate) enum Command {
    Keep,
    FileInto { mailbox: StringItem },
    Redirect { address: StringItem },
    Discard,
    Stop,
    Invalid { command: String },
}
