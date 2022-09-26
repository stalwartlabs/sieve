#[derive(Debug, PartialEq, Eq)]
pub(crate) enum StringItem {
    Text(String),
    VariableName(String),
    VariableNumber(usize),
    MatchMany(usize),
    MatchOne,
    List(Vec<StringItem>),
}
