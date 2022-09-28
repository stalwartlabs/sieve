use serde::{Deserialize, Serialize};

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum StringItem {
    Text(Vec<u8>),
    VariableName(String),
    VariableNumber(usize),
    MatchMany(usize),
    MatchOne,
    List(Vec<StringItem>),
}
