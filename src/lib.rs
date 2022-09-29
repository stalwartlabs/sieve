use compiler::grammar::{capability::Capability, command::Command};
use serde::{Deserialize, Serialize};

pub mod compiler;
pub mod runtime;

#[derive(Debug, Serialize, Deserialize)]
pub struct Sieve {
    capabilities: Vec<Capability>,
    commands: Vec<Command>,
}

pub struct Compiler {
    // Settings
    pub(crate) max_script_len: usize,
    pub(crate) max_string_len: usize,
    pub(crate) max_variable_len: usize,
    pub(crate) max_nested_blocks: usize,
    pub(crate) max_nested_tests: usize,
}

#[cfg(test)]
mod tests {
    use std::{
        collections::{BTreeSet, HashSet},
        fs,
        path::PathBuf,
    };

    use super::*;

    fn read_dir(path: PathBuf, files: &mut Vec<PathBuf>) {
        for entry in fs::read_dir(path).unwrap() {
            let entry = entry.unwrap().path();
            if entry.is_dir() {
                read_dir(entry, files);
            } else if ["svtest", "sieve"]
                .contains(&entry.extension().and_then(|e| e.to_str()).unwrap_or(""))
            {
                files.push(entry);
            }
        }
    }

    #[test]
    fn parse_all() {
        //let mut files = Vec::new();
        //let mut items = BTreeSet::new();

        //read_dir(PathBuf::from("tests"), &mut files);
        //for file in files {
        /*println!("parsing {:?}", file);
        let bytes = fs::read(&file).unwrap();
        let tokens = tokenize(&bytes).unwrap();
        for token in tokens {
            if let Token::Identifier(id) = token.token {
                //items.insert(id.to_lowercase());
            }
        }*/

        /*if file.as_os_str().to_str().unwrap().contains("lexer.svtest") {
            println!("{:#?}", tokens);
            break;
        }*/
        //}

        /*fs::write(
            "identifiers.txt",
            items.into_iter().collect::<Vec<_>>().join("\n"),
        )
        .unwrap();*/
    }
}
