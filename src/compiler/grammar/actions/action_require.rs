use crate::compiler::{
    grammar::{
        instruction::{CompilerState, Instruction},
        Capability,
    },
    lexer::Token,
    CompileError,
};

impl<'x> CompilerState<'x> {
    fn add_capability(&mut self, capabilities: &mut Vec<Capability>, capability: Capability) {
        if !self.has_capability(&capability) {
            let parent_capability = if matches!(&capability, Capability::SpamTestPlus) {
                Some(Capability::SpamTest)
            } else {
                None
            };
            capabilities.push(capability.clone());
            self.block.capabilities.insert(capability);

            if let Some(capability) = parent_capability {
                if !self.has_capability(&capability) {
                    capabilities.push(capability.clone());
                    self.block.capabilities.insert(capability);
                }
            }
        }
    }

    pub(crate) fn parse_require(&mut self) -> Result<(), CompileError> {
        let mut capabilities = Vec::new();

        let token_info = self.tokens.unwrap_next()?;
        match token_info.token {
            Token::BracketOpen => loop {
                let token_info = self.tokens.unwrap_next()?;
                match token_info.token {
                    Token::StringConstant(value) => {
                        self.add_capability(
                            &mut capabilities,
                            Capability::parse(std::str::from_utf8(&value).unwrap_or("")),
                        );
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
                self.add_capability(
                    &mut capabilities,
                    Capability::parse(std::str::from_utf8(&value).unwrap_or("")),
                );
            }
            _ => {
                return Err(token_info.expected("'[' or string"));
            }
        }

        if !capabilities.is_empty() {
            if let Some(Instruction::Require(capabilties)) = self.instructions.last_mut() {
                capabilties.extend(capabilities)
            } else {
                self.instructions.push(Instruction::Require(capabilities));
            }
        }

        Ok(())
    }
}
