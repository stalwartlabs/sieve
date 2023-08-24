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

use crate::{
    compiler::{grammar::tests::test_plugin::Plugin, Value, VariableType},
    Context, Event, PluginArgument,
};

use super::Variable;

impl<'x> Context<'x> {
    pub(crate) fn variable(&self, var: &VariableType) -> Option<&Variable<'x>> {
        match var {
            VariableType::Local(var_num) => self.vars_local.get(*var_num),
            VariableType::Match(var_num) => self.vars_match.get(*var_num),
            VariableType::Global(var_name) => self.vars_global.get(var_name),
            VariableType::Environment(var_name) => self
                .vars_env
                .get(var_name)
                .or_else(|| self.runtime.environment.get(var_name)),
            VariableType::Envelope(envelope) => {
                self.envelope
                    .iter()
                    .find_map(|(e, v)| if e == envelope { Some(v) } else { None })
            }
        }
    }

    pub(crate) fn eval_value<'z: 'y, 'y>(&'z self, string: &'y Value) -> Variable<'y> {
        match string {
            Value::Text(text) => Variable::String(text.into()),
            Value::Variable(var) => self
                .variable(var)
                .map(|value| match value {
                    Variable::String(s) => Variable::StringRef(s.as_str()),
                    Variable::StringRef(s) => Variable::StringRef(s),
                    Variable::Integer(n) => Variable::Integer(*n),
                    Variable::Float(n) => Variable::Float(*n),
                })
                .unwrap_or_default(),
            Value::List(list) => {
                let mut data = String::new();
                for item in list {
                    match item {
                        Value::Text(string) => {
                            data.push_str(string);
                        }
                        Value::Variable(var) => {
                            if let Some(value) = self.variable(var) {
                                data.push_str(&value.to_cow());
                            }
                        }
                        Value::List(_) => {
                            debug_assert!(false, "This should not have happened: {string:?}");
                        }
                        Value::Number(n) => {
                            data.push_str(&n.to_string());
                        }
                        Value::Expression(expr) => {
                            if let Some(value) = self.eval_expression(expr) {
                                data.push_str(&value.to_string());
                            }
                        }
                        Value::Regex(_) => (),
                    }
                }
                data.into()
            }
            Value::Number(n) => Variable::from(*n),
            Value::Expression(expr) => self
                .eval_expression(expr)
                .map(Variable::from)
                .unwrap_or(Variable::default()),
            Value::Regex(r) => Variable::StringRef(&r.expr),
        }
    }

    #[inline(always)]
    pub(crate) fn eval_values<'z: 'y, 'y>(&'z self, strings: &'y [Value]) -> Vec<Variable<'y>> {
        strings.iter().map(|s| self.eval_value(s)).collect()
    }

    #[inline(always)]
    pub(crate) fn eval_values_owned(&self, strings: &[Value]) -> Vec<String> {
        strings
            .iter()
            .map(|s| self.eval_value(s).into_cow().into_owned())
            .collect()
    }

    pub(crate) fn eval_plugin_arguments(&self, plugin: &Plugin) -> Event {
        let mut arguments = Vec::with_capacity(plugin.arguments.len());
        for argument in &plugin.arguments {
            arguments.push(match argument {
                PluginArgument::Tag(tag) => PluginArgument::Tag(*tag),
                PluginArgument::Text(t) => PluginArgument::Text(self.eval_value(t).into_string()),
                PluginArgument::Number(n) => PluginArgument::Number(self.eval_value(n).to_number()),
                PluginArgument::Regex(r) => PluginArgument::Regex(r.clone()),
                PluginArgument::Array(a) => {
                    let mut arr = Vec::with_capacity(a.len());
                    for item in a {
                        arr.push(match item {
                            PluginArgument::Tag(tag) => PluginArgument::Tag(*tag),
                            PluginArgument::Text(t) => {
                                PluginArgument::Text(self.eval_value(t).into_string())
                            }
                            PluginArgument::Number(n) => {
                                PluginArgument::Number(self.eval_value(n).to_number())
                            }
                            PluginArgument::Regex(r) => PluginArgument::Regex(r.clone()),
                            PluginArgument::Array(_) => continue,
                        });
                    }
                    PluginArgument::Array(arr)
                }
            });
        }

        Event::Plugin {
            id: plugin.id,
            arguments,
        }
    }
}

pub(crate) trait IntoString: Sized {
    fn into_string(self) -> String;
}

impl IntoString for Vec<u8> {
    fn into_string(self) -> String {
        String::from_utf8(self)
            .unwrap_or_else(|err| String::from_utf8_lossy(err.as_bytes()).into_owned())
    }
}
