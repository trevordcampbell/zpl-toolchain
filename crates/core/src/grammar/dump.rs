use super::ast::Ast;

/// Serialize an AST to a pretty-printed JSON string.
pub fn to_pretty_json(ast: &Ast) -> String {
    serde_json::to_string_pretty(ast).expect("Ast serialization cannot fail")
}
