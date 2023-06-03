use std::borrow::Cow;
use std::fmt;

use crate::exceptions::{exc, exc_err};
use crate::exceptions::{Exception, InternalRunError};
use crate::object::Object;
use crate::run::RunResult;
use crate::types::{Builtins, CmpOperator, Expr, ExprLoc, Function, Kwarg, Operator};

pub(crate) struct Evaluator<'d> {
    namespace: &'d [Object],
}

impl<'d> Evaluator<'d> {
    pub fn new(namespace: &'d [Object]) -> Self {
        Self { namespace }
    }

    pub fn evaluate<'c>(&self, expr_loc: &'d ExprLoc<'c>) -> RunResult<'c, Cow<'d, Object>> {
        match &expr_loc.expr {
            Expr::Constant(object) => Ok(Cow::Borrowed(object)),
            Expr::Name(ident) => {
                if let Some(object) = self.namespace.get(ident.id) {
                    match object {
                        Object::Undefined => Err(InternalRunError::Undefined(ident.name.clone().into()).into()),
                        _ => Ok(Cow::Borrowed(object)),
                    }
                } else {
                    let name = ident.name.clone();
                    Err(Exception::NameError(name.into())
                        .with_position(expr_loc.position)
                        .into())
                }
            }
            Expr::Call { func, args, kwargs } => Ok(self.call_function(func, args, kwargs)?),
            Expr::Op { left, op, right } => self.op(left, op, right),
            Expr::CmpOp { left, op, right } => Ok(Cow::Owned(self.cmp_op(left, op, right)?.into())),
            Expr::List(elements) => {
                let objects = elements
                    .iter()
                    .map(|e| self.evaluate(e).map(|ob| ob.into_owned()))
                    .collect::<RunResult<_>>()?;
                Ok(Cow::Owned(Object::List(objects)))
            }
        }
    }

    pub fn evaluate_bool<'c>(&self, expr_loc: &'d ExprLoc<'c>) -> RunResult<'c, bool> {
        match &expr_loc.expr {
            Expr::CmpOp { left, op, right } => self.cmp_op(left, op, right),
            _ => self.evaluate(expr_loc)?.as_ref().bool(),
        }
    }

    fn op<'c>(
        &self,
        left: &'d ExprLoc<'c>,
        op: &'d Operator,
        right: &'d ExprLoc<'c>,
    ) -> RunResult<'c, Cow<'d, Object>> {
        let left_object = self.evaluate(left)?;
        let right_object = self.evaluate(right)?;
        let op_object: Option<Object> = match op {
            Operator::Add => left_object.add(&right_object),
            Operator::Sub => left_object.sub(&right_object),
            Operator::Mod => left_object.modulo(&right_object),
            _ => return exc_err!(InternalRunError::TodoError; "Operator {op:?} not yet implemented"),
        };
        match op_object {
            Some(object) => Ok(Cow::Owned(object)),
            None => self._op_type_error(left, op, right, left_object, right_object),
        }
    }

    fn cmp_op<'c>(&self, left: &'d ExprLoc<'c>, op: &'d CmpOperator, right: &'d ExprLoc<'c>) -> RunResult<'c, bool> {
        let left_object = self.evaluate(left)?;
        let right_object = self.evaluate(right)?;
        let op_object: Option<bool> = match op {
            CmpOperator::Eq => left_object.as_ref().eq(&right_object),
            CmpOperator::NotEq => left_object.as_ref().eq(&right_object).map(|object| !object),
            CmpOperator::Gt => Some(left_object.gt(&right_object)),
            CmpOperator::GtE => Some(left_object.ge(&right_object)),
            CmpOperator::Lt => Some(left_object.lt(&right_object)),
            CmpOperator::LtE => Some(left_object.le(&right_object)),
            _ => return exc_err!(InternalRunError::TodoError; "Operator {op:?} not yet implemented"),
        };
        match op_object {
            Some(object) => Ok(object),
            None => self._op_type_error(left, op, right, left_object, right_object),
        }
    }

    fn _op_type_error<'c, T>(
        &self,
        left: &'d ExprLoc<'c>,
        op: impl fmt::Display,
        right: &'d ExprLoc<'c>,
        left_object: Cow<'d, Object>,
        right_object: Cow<'d, Object>,
    ) -> RunResult<'c, T> {
        let left_type = left_object.type_str();
        let right_type = right_object.type_str();
        let new_position  = left.position.extend(&right.position);
        Err(
            exc!(Exception::TypeError; "unsupported operand type(s) for {op}: '{left_type}' and '{right_type}'")
                .with_position(new_position)
                .into(),
        )
    }

    fn call_function<'c>(
        &self,
        function: &'d Function,
        args: &'d [ExprLoc<'c>],
        _kwargs: &'d [Kwarg],
    ) -> RunResult<'c, Cow<'d, Object>> {
        let builtin = match function {
            Function::Builtin(builtin) => builtin,
            Function::Ident(_) => {
                return exc_err!(InternalRunError::TodoError; "User defined functions not yet implemented")
            }
        };
        match builtin {
            Builtins::Print => {
                for (i, arg) in args.iter().enumerate() {
                    let object = self.evaluate(arg)?;
                    if i == 0 {
                        print!("{object}");
                    } else {
                        print!(" {object}");
                    }
                }
                println!();
                Ok(Cow::Owned(Object::None))
            }
            Builtins::Range => {
                if args.len() != 1 {
                    exc_err!(InternalRunError::TodoError; "range() takes exactly one argument")
                } else {
                    let object = self.evaluate(&args[0])?;
                    let size = object.as_int()?;
                    Ok(Cow::Owned(Object::Range(size)))
                }
            }
            Builtins::Len => {
                if args.len() != 1 {
                    exc_err!(Exception::TypeError; "len() takes exactly exactly one argument ({} given)", args.len())
                } else {
                    let object = self.evaluate(&args[0])?;
                    match object.len() {
                        Some(len) => Ok(Cow::Owned(Object::Int(len as i64))),
                        None => exc_err!(Exception::TypeError; "Object of type {} has no len()", object),
                    }
                }
            }
        }
    }
}
