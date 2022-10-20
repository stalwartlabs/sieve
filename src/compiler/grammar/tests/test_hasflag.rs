use serde::{Deserialize, Serialize};

use crate::compiler::{
    grammar::{actions::action_set::Variable, instruction::CompilerState, Capability, Comparator},
    lexer::{string::StringItem, tokenizer::TokenInfo, word::Word, Token},
    CompileError,
};

use crate::compiler::grammar::{test::Test, MatchType};

/*
   Usage: hasflag [MATCH-TYPE] [COMPARATOR]
          [<variable-list: string-list>]
          <list-of-flags: string-list>
*/

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct TestHasFlag {
    pub comparator: Comparator,
    pub match_type: MatchType,
    pub variable_list: Vec<Variable>,
    pub flags: Vec<StringItem>,
    pub is_not: bool,
}

impl<'x> CompilerState<'x> {
    pub(crate) fn parse_test_hasflag(&mut self) -> Result<Test, CompileError> {
        let mut match_type = MatchType::Is;
        let mut comparator = Comparator::AsciiCaseMap;

        let maybe_variables;

        loop {
            let token_info = self.tokens.unwrap_next()?;
            match token_info.token {
                Token::Tag(
                    word @ (Word::Is
                    | Word::Contains
                    | Word::Matches
                    | Word::Value
                    | Word::Count
                    | Word::Regex),
                ) => {
                    self.validate_argument(
                        1,
                        match word {
                            Word::Value | Word::Count => Capability::Relational.into(),
                            Word::Regex => Capability::Regex.into(),
                            Word::List => Capability::ExtLists.into(),
                            _ => None,
                        },
                        token_info.line_num,
                        token_info.line_pos,
                    )?;

                    match_type = self.parse_match_type(word)?;
                }
                Token::Tag(Word::Comparator) => {
                    self.validate_argument(2, None, token_info.line_num, token_info.line_pos)?;
                    comparator = self.parse_comparator()?;
                }
                _ => {
                    maybe_variables = self.parse_strings_token(token_info)?;
                    break;
                }
            }
        }

        match self.tokens.peek() {
            Some(Ok(TokenInfo {
                token: Token::StringConstant(_) | Token::StringVariable(_) | Token::BracketOpen,
                line_num,
                line_pos,
            })) => {
                if !maybe_variables.is_empty() {
                    let line_num = *line_num;
                    let line_pos = *line_pos;

                    let mut variable_list = Vec::with_capacity(maybe_variables.len());
                    for variable in maybe_variables {
                        match variable {
                            StringItem::Text(var_name) => {
                                variable_list.push(self.register_variable(var_name).map_err(
                                    |error_type| CompileError {
                                        line_num,
                                        line_pos,
                                        error_type,
                                    },
                                )?);
                            }
                            _ => {
                                return Err(self
                                    .tokens
                                    .unwrap_next()?
                                    .invalid("variable name has to be a constant"))
                            }
                        }
                    }
                    let flags = self.parse_strings()?;
                    self.validate_match(&match_type, &flags)?;

                    Ok(Test::HasFlag(TestHasFlag {
                        comparator,
                        match_type,
                        variable_list,
                        flags,
                        is_not: false,
                    }))
                } else {
                    Err(self
                        .tokens
                        .unwrap_next()?
                        .invalid("variable name cannot be a list"))
                }
            }
            _ => {
                self.validate_match(&match_type, &maybe_variables)?;

                Ok(Test::HasFlag(TestHasFlag {
                    comparator,
                    match_type,
                    variable_list: Vec::new(),
                    flags: maybe_variables,
                    is_not: false,
                }))
            }
        }
    }
}
