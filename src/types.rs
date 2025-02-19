use crate::ast::Node;
use name_variant::NamedVariant;

#[derive(NamedVariant, Clone)]
pub enum Type {
    Number,
    Float,
    Boolean,

    Error,            // Type is known to be invalid
    Unresolved(Node), // Type is unknown but possibly valid
}

impl Type {
    pub fn try_from_str(text: &str) -> Option<Self> {
        Some(match text {
            "number" => Type::Number,
            "float" => Type::Float,
            "boolean" => Type::Boolean,
            _ => return None,
        })
    }
}

impl PartialEq for Type {
    fn eq(&self, other: &Self) -> bool {
        std::mem::discriminant(self) == std::mem::discriminant(other)
    }
}
