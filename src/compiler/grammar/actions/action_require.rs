use crate::compiler::{
    grammar::{
        command::{Command, CompilerState},
        Capability,
    },
    lexer::Token,
    CompileError,
};

impl<'x> CompilerState<'x> {
    pub(crate) fn parse_require(&mut self) -> Result<(), CompileError> {
        let capabilities = if let Some(Command::Require(capabilties)) = self.commands.last_mut() {
            capabilties
        } else {
            self.commands.push(Command::Require(vec![]));
            if let Some(Command::Require(capabilties)) = self.commands.last_mut() {
                capabilties
            } else {
                unreachable!();
            }
        };

        let token_info = self.tokens.unwrap_next()?;
        match token_info.token {
            Token::BracketOpen => loop {
                let token_info = self.tokens.unwrap_next()?;
                match token_info.token {
                    Token::StringConstant(value) => {
                        let capability = Capability::parse(value);
                        if !capabilities.contains(&capability) {
                            capabilities.push(capability);
                        }
                        let token_info = self.tokens.unwrap_next()?;
                        match token_info.token {
                            Token::Comma => (),
                            Token::BracketClose => break,
                            _ => {
                                return Err(token_info.expected("']' or ','"));
                            }
                        }
                    }
                    _ => {
                        return Err(token_info.expected("string"));
                    }
                }
            },
            Token::StringConstant(value) => {
                let capability = Capability::parse(value);
                if !capabilities.contains(&capability) {
                    capabilities.push(capability);
                }
            }
            _ => {
                return Err(token_info.expected("'[' or string"));
            }
        }

        Ok(())
    }
}
