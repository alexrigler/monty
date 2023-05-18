use ahash::AHashMap;
use std::borrow::Cow;
use crate::evaluate::Evaluator;

use crate::types::{Builtins, Expr, Node};
use crate::object::Object;

pub(crate) type PrepareResult<T> = Result<T, Cow<'static, str>>;

pub(crate) type RunNode = Node<usize, Builtins>;
pub(crate) type RunExpr = Expr<usize, Builtins>;

/// TODO:
/// * check variables exist before pre-assigning
pub(crate) fn prepare(nodes: Vec<Node<String, String>>, input_names: &[&str]) -> PrepareResult<(Vec<Object>, Vec<RunNode>)> {
    let mut p = Prepare::new(nodes.len(), input_names);
    let new_nodes = p.prepare_nodes(nodes)?;
    Ok((p.namespace, new_nodes))
}

struct Prepare {
    name_map: AHashMap<String, usize>,
    namespace: Vec<Object>,
}

impl Prepare {
    fn new(capacity: usize, input_names: &[&str]) -> Self {
        let mut name_map = AHashMap::with_capacity(capacity);
        for (index, name) in input_names.iter().enumerate() {
            name_map.insert(name.to_string(), index);
        }
        let namespace = vec![Object::Undefined; name_map.len()];
        Self { name_map, namespace }
    }

    fn prepare_nodes(&mut self, nodes: Vec<Node<String, String>>) -> PrepareResult<Vec<RunNode>> {
        let mut new_nodes = Vec::with_capacity(nodes.len());
        for node in nodes {
            match node {
                Node::Pass => (),
                Node::Expr(expr) => {
                    let expr = self.prepare_expression(expr)?;
                    new_nodes.push(Node::Expr(expr));
                }
                Node::Assign { target, object } => {
                    let expr = self.prepare_expression(*object)?;
                    let target = self.get_id(target);
                    // if expr.is_const() {
                    //     self.namespace[target] = expr.into_const();
                    // } else {
                    // }
                    new_nodes.push(Node::Assign { target, object: Box::new(expr) });
                }
                Node::OpAssign { target, op, object } => {
                    let target = self.get_id(target);
                    let object = Box::new(self.prepare_expression(*object)?);
                    new_nodes.push(Node::OpAssign { target, op, object });
                }
                Node::For {
                    target,
                    iter,
                    body,
                    or_else,
                } => new_nodes.push(Node::For {
                    target: self.prepare_expression(target)?,
                    iter: self.prepare_expression(iter)?,
                    body: self.prepare_nodes(body)?,
                    or_else: self.prepare_nodes(or_else)?,
                }),
                Node::If { test, body, or_else } => new_nodes.push(Node::If {
                    test: self.prepare_expression(test)?,
                    body: self.prepare_nodes(body)?,
                    or_else: self.prepare_nodes(or_else)?,
                }),
            }
        }
        Ok(new_nodes)
    }

    fn prepare_expression(&mut self, expr: Expr<String, String>) -> PrepareResult<RunExpr> {
        let expr = match expr {
            Expr::Constant(object) => Expr::Constant(object),
            Expr::Name(name) => Expr::Name(self.get_id(name)),
            Expr::Op { left, op, right } => Expr::Op {
                left: Box::new(self.prepare_expression(*left)?),
                op,
                right: Box::new(self.prepare_expression(*right)?),
            },
            Expr::CmpOp { left, op, right } => Expr::CmpOp {
                left: Box::new(self.prepare_expression(*left)?),
                op,
                right: Box::new(self.prepare_expression(*right)?),
            },
            Expr::Call { func, args, kwargs } => {
                let func = Builtins::find(&func)?;
                Expr::Call {
                    func,
                    args: args
                        .into_iter()
                        .map(|e| self.prepare_expression(e))
                        .collect::<PrepareResult<Vec<_>>>()?,
                    kwargs: kwargs
                        .into_iter()
                        .map(|(_, e)| self.prepare_expression(e).map(|e| (0, e)))
                        .collect::<PrepareResult<Vec<_>>>()?,
                }
            }
            Expr::List(elements) => {
                let expressions = elements
                    .into_iter()
                    .map(|e| self.prepare_expression(e))
                    .collect::<PrepareResult<Vec<_>>>()?;
                Expr::List(expressions)
            }
        };

        let evaluate = Evaluator::new(&self.namespace);

        if evaluate.can_be_const(&expr) {
            let object = evaluate.evaluate(&expr)?;
            Ok(Expr::Constant(object.into_owned()))
        } else {
            Ok(expr)
        }
    }

    fn get_id(&mut self, name: String) -> usize {
        *self.name_map.entry(name).or_insert_with(|| {
            let id = self.namespace.len();
            self.namespace.push(Object::Undefined);
            id
        })
    }
}
