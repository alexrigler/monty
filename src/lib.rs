mod parse_error;
mod evaluate;
mod object;
mod parse;
mod prepare;
mod run;
mod types;

pub use crate::object::Object;
use crate::parse::parse;
pub use crate::parse_error::{ParseError, ParseResult};
use crate::prepare::prepare;
use crate::run::{Frame, RunResult};
pub use crate::types::Exit;
use crate::types::Node;

#[derive(Debug)]
pub struct Executor {
    initial_namespace: Vec<Object>,
    nodes: Vec<Node>,
}

impl Executor {
    pub fn new(code: &str, filename: &str, input_names: &[&str]) -> ParseResult<Self> {
        let nodes = parse(code, filename)?;
        // dbg!(&nodes);
        let (initial_namespace, nodes) = prepare(nodes, input_names)?;
        // dbg!(&initial_namespace, &nodes);
        Ok(Self {
            initial_namespace,
            nodes,
        })
    }

    pub fn run(&self, inputs: Vec<Object>) -> RunResult<Exit> {
        let mut namespace = self.initial_namespace.clone();
        for (i, input) in inputs.into_iter().enumerate() {
            namespace[i] = input;
        }
        Frame::new(namespace).execute(&self.nodes)
    }
}

/// parse code and show the parsed AST, mostly for testing
pub fn parse_show(code: &str, filename: &str) -> Result<String, String> {
    match parse(code, filename) {
        Ok(ast) => Ok(format!("{:#?}", ast)),
        Err(e) => Err(e.to_string()),
    }
}
