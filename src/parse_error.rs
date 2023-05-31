use std::borrow::Cow;
use std::fmt;

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum ParseError {
    Todo(&'static str),
    Parsing(String),
    PreEval(String),  // TODO replace with Exception
    Internal(Cow<'static, str>),
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Todo(s) => write!(f, "TODO: {s}"),
            Self::Parsing(s) => write!(f, "Error parsing AST: {s}"),
            Self::PreEval(s) => write!(f, "Pre eval error: {s}"),
            Self::Internal(s) => write!(f, "Internal parsing error: {s}"),
        }
    }
}

impl ParseError {
    pub(crate) fn pre_eval(exception: Cow<'static, str>) -> Self {
        Self::PreEval(exception.to_string())
    }
}

pub type ParseResult<T> = Result<T, ParseError>;
