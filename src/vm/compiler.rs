use crate::ast::{Node, NodeKind, Operator};
use crate::vm::bytecode::{Chunk, OpCode};
use crate::vm::Value;

pub struct Compiler {
    pub chunk: Chunk,
}

impl Compiler {
    pub fn new() -> Self {
        Self {
            chunk: Chunk::new(),
        }
    }

    pub fn compile_program(&mut self, program: &Node) {
        let NodeKind::Block(stmts) = &program.kind else {
            unreachable!()
        };
        for stmt in stmts {
            self.compile(stmt);
        }
        if crate::ARGS.show_bytecode() {
            self.chunk.disassemble();
        }
    }

    pub fn compile(&mut self, node: &Node) {
        match &node.kind {
            NodeKind::Return(val) => {
                self.compile(val);
                self.chunk.write_op(OpCode::Return);
            }
            NodeKind::Block(_) => unimplemented!("awaiting scopes"),
            NodeKind::VarDeclaration(_, _) => unimplemented!("awaiting scopes"),
            NodeKind::UnaryOperation(op, val) => {
                self.compile(val);
                self.chunk.write_op(match op {
                    Operator::Not => OpCode::Not,
                    _ => unreachable!(),
                })
            }
            NodeKind::BinaryOperation(op, lhs, rhs) => {
                self.compile(&lhs);
                self.compile(&rhs);
                self.chunk.write_op(match op {
                    Operator::Plus => OpCode::Add,
                    Operator::Minus => OpCode::Sub,
                    Operator::Star => OpCode::Mul,
                    Operator::Slash => OpCode::Div,
                    Operator::Or => OpCode::Or,
                    Operator::And => OpCode::And,
                    Operator::GreaterThan => OpCode::Greater,
                    Operator::LessThan => OpCode::Less,
                    Operator::GreaterThanEquals => OpCode::Less, // Swap direction and invert result
                    Operator::LessThanEquals => OpCode::Greater,
                    Operator::Equals => OpCode::Equal,
                    Operator::BangEquals => OpCode::Equal,
                    _ => unreachable!(),
                });
                match op {
                    Operator::GreaterThanEquals
                    | Operator::LessThanEquals
                    | Operator::BangEquals => self.chunk.write_op(OpCode::Not),
                    _ => (),
                }
            }
            NodeKind::Identifier(_) => unimplemented!("awaiting var declaration"),
            NodeKind::StringLiteral(val) => self.chunk.write_const(Value::String(val.clone())),
            NodeKind::FloatLiteral(val) => self.chunk.write_const(Value::Float(*val)),
            NodeKind::IntegerLiteral(val) => self.chunk.write_const(Value::Integer(*val as isize)),
            NodeKind::BooleanLiteral(val) => self.chunk.write_const(Value::Boolean(*val)),
        }
    }
}
