/*
 * Copyright (c) 2020-2023, Stalwart Labs Ltd.
 *
 * This file is part of the Stalwart Sieve Interpreter.
 *
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU Affero General Public License as
 * published by the Free Software Foundation, either version 3 of
 * the License, or (at your option) any later version.
 *
 * This program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
 * GNU Affero General Public License for more details.
 * in the LICENSE file at the top-level directory of this distribution.
 * You should have received a copy of the GNU Affero General Public License
 * along with this program.  If not, see <http://www.gnu.org/licenses/>.
 *
 * You can be released from the requirements of the AGPLv3 license by
 * purchasing a commercial license. Please contact licensing@stalw.art
 * for more details.
*/

use std::fmt::Display;

use serde::{Deserialize, Serialize};

use crate::compiler::grammar::instruction::{CompilerState, Instruction, MapLocalVars};
use crate::compiler::lexer::Token;
use crate::compiler::{CompileError, Regex};
use crate::compiler::{ErrorType, Value};
use crate::{ExternalId, PluginArgument, PluginSchema, PluginSchemaArgument};

use crate::compiler::grammar::test::Test;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Plugin {
    pub id: ExternalId,
    pub arguments: Vec<PluginArgument<Value, Value>>,
    pub is_not: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Error {
    pub message: Value,
}

impl<'x> CompilerState<'x> {
    pub(crate) fn parse_plugin(&mut self, schema: &PluginSchema) -> Result<(), CompileError> {
        let instruction = Instruction::Plugin(self.parse_plugin_(schema)?);
        self.tokens.expect_token(Token::Semicolon)?;
        self.instructions.push(instruction);
        Ok(())
    }

    pub(crate) fn parse_test_plugin(
        &mut self,
        schema: &PluginSchema,
    ) -> Result<Test, CompileError> {
        Ok(Test::Plugin(self.parse_plugin_(schema)?))
    }

    fn parse_plugin_(&mut self, schema: &PluginSchema) -> Result<Plugin, CompileError> {
        let mut plugin = Plugin {
            id: schema.id,
            arguments: vec![],
            is_not: false,
        };
        let mut schema_args = schema.arguments.iter();

        while let Some(token_info) = self.tokens.peek() {
            let token_info = token_info?;
            let schema_arg = match &token_info.token {
                Token::Tag(tag) => {
                    let tag = tag.to_string();
                    let token_info = self.tokens.unwrap_next()?;
                    if let Some(tagged_arg) = schema.tags.get(&tag) {
                        plugin.arguments.push(PluginArgument::Tag(tagged_arg.id));
                        if let Some(schema_arg) = &tagged_arg.argument {
                            schema_arg
                        } else {
                            continue;
                        }
                    } else {
                        return Err(token_info.expected("a valid argument"));
                    }
                }
                Token::Unknown(tag) => {
                    if let Some(tagged_arg) =
                        tag.strip_prefix(':').and_then(|tag| schema.tags.get(tag))
                    {
                        self.tokens.unwrap_next()?;
                        plugin.arguments.push(PluginArgument::Tag(tagged_arg.id));
                        if let Some(schema_arg) = &tagged_arg.argument {
                            schema_arg
                        } else {
                            continue;
                        }
                    } else {
                        return Err(self.tokens.unwrap_next()?.expected("a valid argument"));
                    }
                }
                _ => {
                    if let Some(schema_arg) = schema_args.next() {
                        schema_arg
                    } else {
                        break;
                    }
                }
            };

            match schema_arg {
                PluginSchemaArgument::Array(item_schema) => {
                    let mut items = vec![];
                    for item in self.parse_strings()? {
                        match item_schema.convert_argument(item) {
                            Ok(arg) => {
                                items.push(arg);
                            }
                            Err(err) => {
                                return Err(self.tokens.unwrap_next()?.custom(err));
                            }
                        }
                    }
                    plugin.arguments.push(PluginArgument::Array(items));
                }
                PluginSchemaArgument::Variable => {
                    let token = self.tokens.unwrap_next()?;
                    plugin.arguments.push(PluginArgument::Variable(
                        self.parse_variable_name(token, false)?,
                    ));
                }
                _ => match schema_arg.convert_argument(self.parse_string()?) {
                    Ok(arg) => {
                        plugin.arguments.push(arg);
                    }
                    Err(err) => {
                        return Err(self.tokens.unwrap_next()?.custom(err));
                    }
                },
            }
        }

        if let Some(schema_arg) = schema_args.next() {
            self.tokens
                .unwrap_next()?
                .expected(format!("expected a {schema_arg}"));
        }

        Ok(plugin)
    }
}

impl PluginSchemaArgument {
    fn convert_argument(&self, value: Value) -> Result<PluginArgument<Value, Value>, ErrorType> {
        match self {
            PluginSchemaArgument::Text => Ok(PluginArgument::Text(value)),
            PluginSchemaArgument::Number => Ok(PluginArgument::Number(value)),
            PluginSchemaArgument::Regex => {
                if let Value::Text(expr) = value {
                    fancy_regex::Regex::new(&expr)
                        .map(|regex| PluginArgument::Regex(Regex { regex, expr }))
                        .map_err(|err| ErrorType::InvalidRegex(err.to_string()))
                } else {
                    Err(ErrorType::InvalidRegex(
                        "Expected a regular expression".to_string(),
                    ))
                }
            }
            _ => Err(ErrorType::InvalidArguments),
        }
    }
}

impl MapLocalVars for PluginArgument<Value, Value> {
    fn map_local_vars(&mut self, last_id: usize) {
        match self {
            PluginArgument::Text(v) => v.map_local_vars(last_id),
            PluginArgument::Number(v) => v.map_local_vars(last_id),
            PluginArgument::Array(v) => v.map_local_vars(last_id),
            PluginArgument::Variable(v) => v.map_local_vars(last_id),
            _ => (),
        }
    }
}

impl Display for PluginSchemaArgument {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PluginSchemaArgument::Text => write!(f, "string"),
            PluginSchemaArgument::Number => write!(f, "number"),
            PluginSchemaArgument::Regex => write!(f, "regular expression"),
            PluginSchemaArgument::Variable => write!(f, "variable"),
            PluginSchemaArgument::Array(item) => write!(f, "array of {}s", item),
        }
    }
}
