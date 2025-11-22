use std::borrow::Cow;
use std::fmt;

use crate::expressions::ExprLoc;
use crate::heap::Heap;
use crate::object::Object;
use crate::parse::CodeRange;
use crate::run::RunResult;

#[allow(clippy::enum_variant_names)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ExcType {
    ValueError,
    TypeError,
    NameError,
    AttributeError,
}

impl fmt::Display for ExcType {
    // TODO replace with a strum
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.str())
    }
}

impl ExcType {
    // TODO replace with a strum
    fn str(self) -> &'static str {
        match self {
            Self::ValueError => "ValueError",
            Self::TypeError => "TypeError",
            Self::NameError => "NameError",
            Self::AttributeError => "AttributeError",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Exception {
    exc_type: ExcType,
    args: Vec<Object>,
}

impl fmt::Display for Exception {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Note: Display trait doesn't have heap access, so Ref objects show as <Ref(id)>
        // For proper formatting, use repr(heap) method instead
        // different output for no args, 1 arg, and more than 1 args
        let mut args_iter = self.args.iter();
        if let Some(first_arg) = args_iter.next() {
            if let Some(second_arg) = args_iter.next() {
                // more than one arg, print as tuple
                write!(f, "({first_arg}, {second_arg}")?;
                for arg in args_iter {
                    write!(f, ", {arg}")?;
                }
                write!(f, ")")
            } else {
                // one arg, simply return it
                write!(f, "{first_arg}")
            }
        } else {
            // no args, nothing is printed
            Ok(())
        }
    }
}

impl Exception {
    pub(crate) fn new(s: String, exc_type: ExcType) -> Self {
        Exception {
            exc_type,
            args: vec![Object::Str(s)],
        }
    }

    pub(crate) fn call(args: Vec<Object>, exc_type: ExcType) -> Self {
        Exception { exc_type, args }
    }

    pub(crate) fn str_with_type(&self) -> String {
        format!("{}: {self}", self.exc_type)
    }

    pub(crate) fn type_str(&self) -> &'static str {
        self.exc_type.str()
    }

    /// Returns a repr string for this exception with heap access for formatting arguments.
    pub fn repr(&self, heap: &Heap) -> String {
        let mut s = self.exc_type.to_string();
        s.push('(');

        let mut args_iter = self.args.iter();
        if let Some(first) = args_iter.next() {
            s.push_str(&first.repr(heap));
            for arg in args_iter {
                s.push_str(", ");
                s.push_str(&arg.repr(heap));
            }
        }
        s.push(')');
        s
    }

    pub(crate) fn with_frame(self, frame: StackFrame) -> ExceptionRaise {
        ExceptionRaise {
            exc: self,
            frame: Some(frame),
        }
    }

    pub(crate) fn with_position(self, position: CodeRange) -> ExceptionRaise {
        ExceptionRaise {
            exc: self,
            frame: Some(StackFrame::from_position(position)),
        }
    }

    pub(crate) fn operand_type_error<'c, 'd, T>(
        left: &'d ExprLoc<'c>,
        op: impl fmt::Display,
        right: &'d ExprLoc<'c>,
        left_object: Cow<'d, Object>,
        right_object: Cow<'d, Object>,
        heap: &crate::heap::Heap,
    ) -> RunResult<'c, T> {
        let left_type = left_object.type_str_with_heap(heap);
        let right_type = right_object.type_str_with_heap(heap);
        let new_position = left.position.extend(&right.position);
        Err(
            exc!(ExcType::TypeError; "unsupported operand type(s) for {op}: '{left_type}' and '{right_type}'")
                .with_position(new_position)
                .into(),
        )
    }
}

macro_rules! exc {
    ($error_type:expr; $msg:tt) => {
        crate::exceptions::Exception::new(format!($msg), $error_type)
    };
    ($error_type:expr; $msg:tt, $( $msg_args:expr ),+ ) => {
        crate::exceptions::Exception::new(format!($msg, $( $msg_args ),+), $error_type)
    };
}
pub(crate) use exc;

macro_rules! exc_err {
    ($error_type:expr; $msg:tt) => {
        Err(crate::exceptions::exc!($error_type; $msg).into())
    };
    ($error_type:expr; $msg:tt, $( $msg_args:expr ),+ ) => {
        Err(crate::exceptions::exc!($error_type; $msg, $( $msg_args ),+).into())
    };
}
pub(crate) use exc_err;

#[derive(Debug, Clone)]
pub struct ExceptionRaise<'c> {
    pub exc: Exception,
    // first in vec is closes "bottom" frame
    pub(crate) frame: Option<StackFrame<'c>>,
}

impl fmt::Display for ExceptionRaise<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(ref frame) = self.frame {
            writeln!(f, "Traceback (most recent call last):")?;
            write!(f, "{frame}")?;
        }
        write!(f, "{}", self.exc.str_with_type())
    }
}

impl From<Exception> for ExceptionRaise<'_> {
    fn from(exc: Exception) -> Self {
        ExceptionRaise { exc, frame: None }
    }
}

impl ExceptionRaise<'_> {
    pub(crate) fn summary(&self) -> String {
        let exc = self.exc.str_with_type();
        if let Some(ref frame) = self.frame {
            format!("({}) {exc}", frame.position)
        } else {
            format!("(<no-tb>) {exc}")
        }
    }
}

#[derive(Debug, Clone)]
pub struct StackFrame<'c> {
    pub(crate) position: CodeRange<'c>,
    pub(crate) frame_name: Option<&'c str>,
    pub(crate) parent: Option<Box<StackFrame<'c>>>,
}

impl fmt::Display for StackFrame<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(ref parent) = self.parent {
            write!(f, "{parent}")?;
        }

        self.position.traceback(f, self.frame_name)
    }
}

impl<'c> StackFrame<'c> {
    pub(crate) fn new(position: &CodeRange<'c>, frame_name: &'c str, parent: Option<&StackFrame<'c>>) -> Self {
        Self {
            position: *position,
            frame_name: Some(frame_name),
            parent: parent.map(|parent| Box::new(parent.clone())),
        }
    }

    fn from_position(position: CodeRange<'c>) -> Self {
        Self {
            position,
            frame_name: None,
            parent: None,
        }
    }
}

#[derive(Debug, Clone)]
pub enum InternalRunError {
    Error(Cow<'static, str>),
    TodoError(Cow<'static, str>),
    // could be NameError, but we don't always have the name
    Undefined(Cow<'static, str>),
}

macro_rules! internal_error {
    ($error_type:expr; $msg:tt) => {
        $error_type(format!($msg).into())
    };
    ($error_type:expr; $msg:tt, $( $msg_args:expr ),+ ) => {
        $error_type(format!($msg, $( $msg_args ),+).into())
    };
}
pub(crate) use internal_error;

macro_rules! internal_err {
    ($error_type:expr; $msg:tt) => {
        Err(crate::exceptions::internal_error!($error_type; $msg).into())
    };
    ($error_type:expr; $msg:tt, $( $msg_args:expr ),+ ) => {
        Err(crate::exceptions::internal_error!($error_type; $msg, $( $msg_args ),+).into())
    };
}
pub(crate) use internal_err;

impl fmt::Display for InternalRunError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Error(s) => write!(f, "Internal Error: {s}"),
            Self::TodoError(s) => write!(f, "Internal Error TODO: {s}"),
            Self::Undefined(s) => {
                if s.is_empty() {
                    write!(f, "Internal Error: accessing undefined object")
                } else {
                    write!(f, "Internal Error: accessing undefined object `{s}`")
                }
            }
        }
    }
}

#[derive(Debug, Clone)]
pub enum RunError<'c> {
    Internal(InternalRunError),
    Exc(ExceptionRaise<'c>),
}

impl fmt::Display for RunError<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Internal(s) => write!(f, "{s}"),
            Self::Exc(s) => write!(f, "{s}"),
        }
    }
}

impl From<InternalRunError> for RunError<'_> {
    fn from(internal_error: InternalRunError) -> Self {
        Self::Internal(internal_error)
    }
}

impl<'c> From<ExceptionRaise<'c>> for RunError<'c> {
    fn from(exc: ExceptionRaise<'c>) -> Self {
        Self::Exc(exc)
    }
}

impl From<Exception> for RunError<'_> {
    fn from(exc: Exception) -> Self {
        Self::Exc(exc.into())
    }
}
