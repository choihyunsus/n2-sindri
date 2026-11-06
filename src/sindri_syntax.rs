//! Inspect actual AST modifiers and report parser locations without echoing source.
use super::{v, Severity, Violation};
use tree_sitter::Node;

const MAX_PARSE_ERRORS:usize=5;

pub(super) fn unsafe_function(node:Node)->bool {
    if node.kind()!="function_item" {return false;}
    let mut cursor=node.walk();
    let found=node.children(&mut cursor).any(|child| {
        if child.kind()!="function_modifiers" {return false;}
        let mut modifiers=child.walk();
        let found=child.children(&mut modifiers).any(|modifier|modifier.kind()=="unsafe");
        found
    });
    found
}

pub(super) fn parse_errors(root:Node,file:&str)->Vec<Violation> {
    let mut stack=vec![root];
    let mut errors=vec![];
    while let Some(node)=stack.pop() {
        if node.is_error()||node.is_missing() {
            let position=node.start_position();
            let kind=if node.is_missing(){"missing token"}else{"unexpected syntax"};
            errors.push(v("PARSE_ERROR",Severity::Blocker,file,position.row+1,
                format!("Parser reported {kind} at column {} ({})",position.column+1,node.kind())));
            if errors.len()==MAX_PARSE_ERRORS {break;}
        }
        let mut cursor=node.walk();
        let children:Vec<_>=node.children(&mut cursor).filter(|child|child.has_error()||child.is_missing()).collect();
        stack.extend(children.into_iter().rev());
    }
    errors
}
