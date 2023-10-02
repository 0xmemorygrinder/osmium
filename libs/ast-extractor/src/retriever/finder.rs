/**
 * finder.rs
 * Function to retrieve contract nodes from position
 * author: 0xSwapFeeder
 */
use syn_solidity::*;
use proc_macro2::LineColumn;
use syn::ExprLit;
use syn_solidity::kw::contract;
use syn_solidity::visit::{visit_expr_new, visit_variable_declaration};
use crate::retriever::finder::find_node::FoundNode;

mod find_node;

macro_rules! is_in_range {
    ($start:expr, $end:expr, $pos:expr) => {
        ($pos.line == $start.line && $pos.char >= $start.column && $start.line != $end.line) ||
        ($pos.line == $end.line && $pos.char <= $end.column && $start.line != $end.line) ||
        ($pos.line == $start.line && $pos.line == $end.line && $pos.char >= $start.column && $pos.char <= $end.column) ||
        ($pos.line > $start.line && $pos.line < $end.line)
    };
}

pub struct Position {
    line: usize,
    char: usize,
}

impl Position {
    pub fn new(line: usize, char: usize) -> Self {
        Self {
            line,
            char,
        }
    }

}

impl Default for Position {
    fn default() -> Self {
        Self {
            line: 0,
            char: 0,
        }
    }
}

struct FinderVisitor {
    current_contract: Option<ItemContract>,
    current_function: Option<ItemFunction>,
    current_property: Option<VariableDefinition>,
    current_variable: Option<VariableDeclaration>,
    current_enum: Option<ItemEnum>,
    current_struct: Option<ItemStruct>,
    current_error: Option<ItemError>,
    current_event: Option<ItemEvent>,
    current_expr: Option<Expr>,
    current_stmt: Option<Stmt>,
    found: Option<FoundNode>,
    to_find: Position,
}


impl FinderVisitor {

    pub fn new(pos: Position) -> Self {
        Self {
            current_contract: None,
            current_function: None,
            current_property: None,
            current_variable: None,
            current_enum: None,
            current_struct: None,
            current_error: None,
            current_event: None,
            current_expr: None,
            current_stmt: None,
            found: None,
            to_find: pos,
        }
    }

    fn check_inheritance_matching(&mut self, contract: &ItemContract) -> bool {
        if let Some(inheritance) = &contract.inheritance {
            if is_in_range!(inheritance.span().start(), inheritance.span().end(), self.to_find) {
                for inherit in &inheritance.inheritance {
                    if is_in_range!(inherit.span().start(), inherit.span().end(), self.to_find) {
                        self.found = Some(FoundNode::ContractDefInheritance(contract.clone(), inherit.clone()));
                        return true;
                    }
                }
            }
        }
        return false;
    }
}

impl<'ast> Visit<'ast> for FinderVisitor {
    fn visit_stmt(&mut self, stmt: &'ast Stmt) {
        if is_in_range!(stmt.span().start(), stmt.span().end(), self.to_find) {
            println!("stmt: {:?}", stmt);
            self.current_stmt = Some(stmt.clone());
            visit::visit_stmt(self, stmt);
        }
    }

    fn visit_expr(&mut self, expr: &'ast Expr) {
        if is_in_range!(expr.span().start(), expr.span().end(), self.to_find) {
            println!("expr: {:?}", expr);
            self.current_expr = Some(expr.clone());
            visit::visit_expr(self, expr);
        }
    }


    fn visit_expr_call(&mut self, call: &'ast ExprCall) {
        if is_in_range!(call.span().start(), call.span().end(), self.to_find) {
            if !is_in_range!(call.args.span().start(), call.args.span().end(), self.to_find) {
                self.found = Some(FoundNode::IdentUsageCall(self.current_contract.clone(), self.current_function.clone(), call.clone()));
            }
            visit::visit_expr_call(self, call);
        }
    }

    //TODO: Found Limitation: cannot check parameter list of a new expr
    // Therefore we can not goto or list_ref any variable used in a new expr
    fn visit_expr_new(&mut self, new: &'ast ExprNew) {
        if is_in_range!(new.ty.span().start(), new.ty.span().end(), self.to_find) {
            self.found = Some(FoundNode::ContractInstantiation(self.current_contract.clone().unwrap().clone(), self.current_function.clone(), new.clone()));
            return;
        }
    }

    fn visit_type(&mut self, ty: &'ast Type) {
        println!("type: {:?}", ty);
        if is_in_range!(ty.span().start(), ty.span().end(), self.to_find) {
            self.found = Some(FoundNode::TypeUsage(self.current_contract.clone(), self.current_function.clone(), self.current_expr.clone(), ty.clone()));
            visit::visit_type(self, ty);
        }
    }

    fn visit_variable_declaration(&mut self, var: &'ast VariableDeclaration) {
        if is_in_range!(var.span().start(), var.span().end(), self.to_find) {
            self.current_variable = Some(var.clone());
            let s = var.name.span().start();
            let e = var.name.span().end();
            if is_in_range!(var.name.span().start(), var.name.span().end(), self.to_find) {
                    self.found = Some(FoundNode::VariableDefName(self.current_contract.clone(), self.current_function.clone(), var.clone(), var.name.clone()));
                    return;
            }
            visit_variable_declaration(self, var);
        }

    }

    fn visit_stmt_var_decl(&mut self, stmt_var_decl: &'ast StmtVarDecl) {
        if is_in_range!(stmt_var_decl.span().start(), stmt_var_decl.span().end(), self.to_find) {
            visit::visit_stmt_var_decl(self, stmt_var_decl);
        }
    }

    fn visit_variable_definition(&mut self, var: &'ast VariableDefinition) {
        if is_in_range!(var.span().start(), var.span().end(), self.to_find) {
            self.current_property = Some(var.clone());
            if is_in_range!(var.name.span().start(), var.name.span().end(), self.to_find) {
                if self.current_contract.is_none() {
                    self.found = Some(FoundNode::ConstantVariableDefName(var.clone(), var.name.clone()))
                } else {
                    self.found = Some(FoundNode::PropertyDefName(self.current_contract.clone().unwrap(),var.clone(), var.name.clone()));
                }
                return;
            }
            visit::visit_variable_definition(self, var);
        }
    }

    fn visit_item_contract(&mut self, contract: &'ast ItemContract) {
        let contract_start = contract.brace_token.span().start();
        let contract_end = contract.brace_token.span().end();
        self.current_contract = Some(contract.clone());
        if is_in_range!(contract.span().start(), contract.span().end(), self.to_find) {
            self.found = Some(FoundNode::ContractDefName(contract.clone()));
        }
        self.check_inheritance_matching(contract);
        if is_in_range!(contract.brace_token.span().start(), contract.brace_token.span().end(), self.to_find) {
            visit::visit_item_contract(self, contract);
        }
        self.current_contract = None;
    }

    fn visit_item_enum(&mut self, enumm: &'ast ItemEnum) {
        self.current_enum = Some(enumm.clone());
        if is_in_range!(enumm.name.span().start(), enumm.name.span().end(), self.to_find) {
            self.found = Some(FoundNode::EnumDefName(self.current_contract.clone(),enumm.clone(), enumm.name.clone()));
            return;
        }
        for variant in &enumm.variants {
            if is_in_range!(variant.ident.span().start(), variant.ident.span().end(), self.to_find) {
                self.found = Some(FoundNode::EnumDefValue(self.current_contract.clone(), enumm.clone(), variant.clone(), variant.ident.clone()));
                return;
            }
        }
        visit::visit_item_enum(self, enumm);
        self.current_enum = None;
    }

    fn visit_item_error(&mut self, error: &'ast ItemError) {
        self.current_error = Some(error.clone());
        if is_in_range!(error.name.span().start(), error.name.span().end(), self.to_find) {
            self.found = Some(FoundNode::ErrorDefName(self.current_contract.clone(), error.clone(), error.name.clone()));
            return;
        }
        for param in &error.parameters {
            if is_in_range!(param.name.span().start(), param.name.span().end(), self.to_find) {
                self.found = Some(FoundNode::ErrorDefParameter(self.current_contract.clone(), error.clone(), param.clone()));
                return;
            }
        }
        visit::visit_item_error(self, error);
        self.current_error = None;

    }

    fn visit_item_event(&mut self, event: &'ast ItemEvent) {
        self.current_event = Some(event.clone());
        if is_in_range!(event.name.span().start(), event.name.span().end(), self.to_find) {
            self.found = Some(FoundNode::EventDefName(self.current_contract.clone().unwrap().clone(), event.clone(), event.name.clone()));
            return;
        }
        for param in &event.parameters {
            if is_in_range!(param.name.span().start(), param.name.span().end(), self.to_find) {
                self.found = Some(FoundNode::EventDefParameter(self.current_contract.clone().unwrap().clone(), event.clone(), param.clone()));
                return;
            }
        }
        visit::visit_item_event(self, event);
        self.current_event = None;
    }

    fn visit_item_function(&mut self, function: &'ast ItemFunction) {
        self.current_function = Some(function.clone());
        if is_in_range!(function.name.span().start(), function.name.span().end(), self.to_find) {
            self.found = Some(FoundNode::FunctionDefName(self.current_contract.clone().unwrap(), function.clone()));
            return;
        }

        if is_in_range!(function.arguments.span().start(), function.arguments.span().end(), self.to_find) {
            for param in &function.arguments {
                if is_in_range!(param.name.span().start(), param.name.span().end(), self.to_find) {
                    self.found = Some(FoundNode::FunctionDefParameterName(self.current_contract.clone().unwrap(), function.clone(), param.clone(), param.name.clone()));
                    break;
                }
            }
        }
        if let FunctionBody::Block(block) = &function.body {
            if is_in_range!(block.span().start(), block.span().end(), self.to_find) {
                visit::visit_item_function(self, function);
            }
        }
        if let Some(ret) = function.return_type() {
            visit::visit_type(self, &ret);
        }
        self.current_function = None;
    }

    fn visit_ident(&mut self, ident: &'ast SolIdent) {
        if self.found.is_some() {
            return;
        }
        if is_in_range!(ident.span().start(), ident.span().end(), self.to_find) {
            self.found = Some(FoundNode::IdentUsageName(self.current_contract.clone(), self.current_function.clone(), self.current_expr.clone(), ident.clone()));
            return;
        }
    }

    fn visit_item_struct(&mut self, strukt: &'ast ItemStruct) {
        self.current_struct = Some(strukt.clone());
        if is_in_range!(strukt.name.span().start(), strukt.name.span().end(), self.to_find) {
            self.found = Some(FoundNode::StructDefName(self.current_contract.clone(), strukt.name.clone()));
            return;
        }
        if is_in_range!(strukt.brace_token.span().start(), strukt.brace_token.span().end(), self.to_find) {
            for field in &strukt.fields {
                if is_in_range!(field.name.span().start(), field.name.span().end(), self.to_find) {
                    self.found = Some(FoundNode::StructDefPropertyName( self.current_contract.clone(), field.clone(), field.name.clone()));
                    return;
                }
            }
            visit::visit_item_struct(self, strukt);
        }
        self.current_struct = None;
    }

}


pub fn retrieve_node_from_position(ast: &File, pos: Position) -> Option<FoundNode> {
    let mut visitor = FinderVisitor::new(pos);
    visitor.visit_file(ast);
    visitor.found
}


#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use proc_macro2::TokenStream;

    use super::*;
    use std::str::FromStr;



    #[test]
    fn test_retrieve_contract_def_name() {
        let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        path.push("tests");
        path.push("files");
        path.push("contracts");
        path.push("one.sol");
        let source = fs::read_to_string(path).unwrap();

        let tokens = TokenStream::from_str(source.as_str()).unwrap();
        let ast = parse2(tokens).unwrap();
        let res = retrieve_node_from_position(&ast, Position::new(1, 20));
        if let Some(node) = res {
            match &node {
                FoundNode::ContractDefName(contract) => {
                    assert_eq!(contract.name.to_string(), "One");
                }
                _ => {
                    panic!()
                }
            }

        } else {
            panic!()
        }
    }

    #[test]
    fn test_retrieve_contract_inheritance() {
        let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        path.push("tests");
        path.push("files");
        path.push("contracts");
        path.push("inheritance.sol");
        let source = fs::read_to_string(path).unwrap();

        let tokens = TokenStream::from_str(source.as_str()).unwrap();
        let ast = parse2(tokens).unwrap();
        let res = retrieve_node_from_position(&ast, Position::new(1, 28));
        if let Some(node) = res {
            match &node {
                FoundNode::ContractDefInheritance(contract, modifier) => {
                    assert_eq!(modifier.name.to_string(), "ERC20");
                }
                _ => {
                    panic!()
                }
            }

        } else {
            panic!()
        }
    }
    #[test]
    fn test_retrieve_contract_inheritance_second() {
        let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        path.push("tests");
        path.push("files");
        path.push("contracts");
        path.push("inheritance_3.sol");
        let source = fs::read_to_string(path).unwrap();

        let tokens = TokenStream::from_str(source.as_str()).unwrap();
        let ast = parse2(tokens).unwrap();
        let res = retrieve_node_from_position(&ast, Position::new(1, 34));
        if let Some(node) = res {
            match &node {
                FoundNode::ContractDefInheritance(contract, modifier) => {
                    assert_eq!(modifier.name.to_string(), "ERC721");
                }
                _ => {
                    panic!()
                }
            }

        } else {
            panic!()
        }
    }

    #[test]
    fn test_retrieve_contract_instantiation() {
        let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        path.push("tests");
        path.push("files");
        path.push("contracts");
        path.push("instantiation.sol");
        let source = fs::read_to_string(path).unwrap();

        let tokens = TokenStream::from_str(source.as_str()).unwrap();
        let ast = parse2(tokens).unwrap();
        let res = retrieve_node_from_position(&ast, Position::new(15, 22));
        println!("{:?}", res);
        if let Some(node) = res {
            match &node {
                FoundNode::ContractInstantiation(contract, func, expr) => {
                     match &expr.ty {
                         Type::Custom(sol_path) => {
                                assert_eq!(sol_path.to_string(), "One");
                         }
                         _ => {
                             panic!()
                         }
                     }
                }
                _ => {
                    panic!()
                }
            }

        } else {
            panic!()
        }
    }

    #[test]
    fn test_retrieve_function_def_name() {
        let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        path.push("tests");
        path.push("files");
        path.push("contracts");
        path.push("one.sol");
        let source = fs::read_to_string(path).unwrap();

        let tokens = TokenStream::from_str(source.as_str()).unwrap();
        let ast = parse2(tokens).unwrap();
        let res = retrieve_node_from_position(&ast, Position::new(3, 14));
        if let Some(node) = res {
            match &node {
                FoundNode::FunctionDefName(_,f) => {
                    if let Some(name) = &f.name {
                        assert_eq!(name.to_string(), "set");
                    } else {
                        panic!()
                    }
                }
                _ => {
                    panic!()
                }
            }

        } else {
            panic!()
        }
    }

    #[test]
    fn test_retrieve_function_usage() {
        let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        path.push("tests");
        path.push("files");
        path.push("functions");
        path.push("internal_call.sol");
        let source = fs::read_to_string(path).unwrap();

        let tokens = TokenStream::from_str(source.as_str()).unwrap();
        let ast = parse2(tokens).unwrap();
        let res = retrieve_node_from_position(&ast, Position::new(9, 18));
        if let Some(node) = res {
            match &node {
                FoundNode::IdentUsageCall(contract, func, expr) => {
                    match &*expr.expr {
                        Expr::Ident(ident) => {
                            assert_eq!(ident.to_string(), "test");
                        }
                        _ => {
                            panic!()
                        }
                    }
                }
                _ => {
                    panic!()
                }
            }

        } else {
            panic!()
        }
    }

    #[test]
    fn test_retrieve_function_param() {
        let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        path.push("tests");
        path.push("files");
        path.push("contracts");
        path.push("one.sol");
        let source = fs::read_to_string(path).unwrap();

        let tokens = TokenStream::from_str(source.as_str()).unwrap();
        let ast = parse2(tokens).unwrap();
        let res = retrieve_node_from_position(&ast, Position::new(3, 23));
        if let Some(node) = res {
            match &node {
                FoundNode::FunctionDefParameterName(contract, func, var, ident) => {
                    if let Some(name) = &ident {
                        assert_eq!(name.to_string(), "x");
                    } else {
                        panic!()
                    }
                }
                _ => {
                    panic!()
                }
            }

        } else {
            panic!()
        }
    }

    #[test]
    fn test_retrieve_prop_def() {
        let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        path.push("tests");
        path.push("files");
        path.push("functions");
        path.push("one.sol");
        let source = fs::read_to_string(path).unwrap();

        let tokens = TokenStream::from_str(source.as_str()).unwrap();
        let ast = parse2(tokens).unwrap();
        let res = retrieve_node_from_position(&ast, Position::new(2, 13));
        if let Some(node) = res {
            match &node {
                FoundNode::PropertyDefName(contract, var, ident) => {
                    assert_eq!(ident.to_string(), "storedData");
                }
                _ => {
                    panic!()
                }
            }

        } else {
            panic!()
        }
    }

    #[test]
    fn test_retrieve_constant_def() {
        let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        path.push("tests");
        path.push("files");
        path.push("constants");
        path.push("constant.sol");
        let source = fs::read_to_string(path).unwrap();

        let tokens = TokenStream::from_str(source.as_str()).unwrap();
        let ast = parse2(tokens).unwrap();
        let res = retrieve_node_from_position(&ast, Position::new(1, 21));
        if let Some(node) = res {
            match &node {
                FoundNode::ConstantVariableDefName(var, ident ) => {
                    assert_eq!(ident.to_string(), "myConst");
                }
                _ => {
                    panic!()
                }
            }

        } else {
            panic!()
        }
    }

    #[test]
    fn test_retrieve_var_def() {
        let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        path.push("tests");
        path.push("files");
        path.push("contracts");
        path.push("two.sol");
        let source = fs::read_to_string(path).unwrap();

        let tokens = TokenStream::from_str(source.as_str()).unwrap();
        let ast = parse2(tokens).unwrap();
        let res = retrieve_node_from_position(&ast, Position::new(4, 17));
        if let Some(node) = res {
            match &node {
                FoundNode::VariableDefName(contract, func, var , ident) => {
                    if let Some(name) = ident {
                        assert_eq!(name.to_string(), "myString");
                    } else {
                        panic!()
                    }
                }
                _ => {
                    panic!()
                }
            }

        } else {
            panic!()
        }
    }


    #[test]
    fn test_retrieve_type_string() {
        let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        path.push("tests");
        path.push("files");
        path.push("contracts");
        path.push("two.sol");
        let source = fs::read_to_string(path).unwrap();

        let tokens = TokenStream::from_str(source.as_str()).unwrap();
        let ast = parse2(tokens).unwrap();
        let res = retrieve_node_from_position(&ast, Position::new(4, 10));
        println!("{:?}", res);
        if let Some(node) = res {
            match &node {
                FoundNode::TypeUsage(_,_,_,ty) => {
                    match ty {
                        Type::String(_) => {assert!(true)}
                        _ => {panic!()}
                    }
                }
                _ => {
                    panic!()
                }
            }

        } else {
            panic!()
        }
    }

    #[test]
    fn test_retrieve_type_call() {
        let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        path.push("tests");
        path.push("files");
        path.push("structs");
        path.push("one.sol");
        let source = fs::read_to_string(path).unwrap();

        let tokens = TokenStream::from_str(source.as_str()).unwrap();
        let ast = parse2(tokens).unwrap();
        let res = retrieve_node_from_position(&ast, Position::new(13, 36));
        println!("{:?}", res);
        if let Some(node) = res {
            match &node {
                FoundNode::IdentUsageCall(_, _, expr) => {
                    match &*expr.expr {
                        Expr::Ident(ident) => {
                            assert_eq!(ident.to_string(), "another_one");
                        }
                        _ => {panic!()}
                    }
                }
                _ => {
                    panic!()
                }
            }

        } else {
            panic!()
        }
    }

    #[test]
    fn test_retrieve_type_custom() {
        let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        path.push("tests");
        path.push("files");
        path.push("structs");
        path.push("one.sol");
        let source = fs::read_to_string(path).unwrap();

        let tokens = TokenStream::from_str(source.as_str()).unwrap();
        let ast = parse2(tokens).unwrap();
        let res = retrieve_node_from_position(&ast, Position::new(13, 12));
        println!("{:?}", res);
        if let Some(node) = res {
            match &node {
                FoundNode::TypeUsage(_,_,expr, ty) => {
                    match ty {
                        Type::Custom(ident) => {
                            assert_eq!(ident.to_string(), "another_one");
                        }
                        _ => {panic!()}
                    }
                }
                _ => {
                    panic!()
                }
            }

        } else {
            panic!()
        }
    }

    #[test]
    fn test_retrieve_type_rturn() {
        let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        path.push("tests");
        path.push("files");
        path.push("contracts");
        path.push("one.sol");
        let source = fs::read_to_string(path).unwrap();

        let tokens = TokenStream::from_str(source.as_str()).unwrap();
        let ast = parse2(tokens).unwrap();
        let res = retrieve_node_from_position(&ast, Position::new(7, 42));
        println!("{:?}", res);
        if let Some(node) = res {
            match &node {
                FoundNode::TypeUsage(_,_,expr, ty) => {
                    match ty {
                        Type::Uint(_, _) => {}
                        _ => {panic!()}
                    }
                }
                _ => {
                    panic!()
                }
            }

        } else {
            panic!()
        }
    }

    #[test]
    fn test_retrieve_contract_struct_def() {
        let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        path.push("tests");
        path.push("files");
        path.push("structs");
        path.push("one.sol");
        let source = fs::read_to_string(path).unwrap();

        let tokens = TokenStream::from_str(source.as_str()).unwrap();
        let ast = parse2(tokens).unwrap();
        let res = retrieve_node_from_position(&ast, Position::new(7, 14));
        println!("{:?}", res);
        if let Some(node) = res {
            match &node {
                FoundNode::StructDefName(contract, ident) => {
                    assert_eq!(contract.is_some(), true);
                    assert_eq!(ident.to_string(), "another_one");
                }
                _ => {
                    panic!()
                }
            }

        } else {
            panic!()
        }
    }

    #[test]
    fn test_retrieve_file_struct_def() {
        let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        path.push("tests");
        path.push("files");
        path.push("structs");
        path.push("one.sol");
        let source = fs::read_to_string(path).unwrap();

        let tokens = TokenStream::from_str(source.as_str()).unwrap();
        let ast = parse2(tokens).unwrap();
        let res = retrieve_node_from_position(&ast, Position::new(1, 10));
        println!("{:?}", res);
        if let Some(node) = res {
            match &node {
                FoundNode::StructDefName(contract, ident) => {
                    assert_eq!(contract.is_none(), true);
                    assert_eq!(ident.to_string(), "one");
                }
                _ => {
                    panic!()
                }
            }

        } else {
            panic!()
        }
    }

    #[test]
    fn test_retrieve_contract_struct_prop() {
        let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        path.push("tests");
        path.push("files");
        path.push("structs");
        path.push("one.sol");
        let source = fs::read_to_string(path).unwrap();

        let tokens = TokenStream::from_str(source.as_str()).unwrap();
        let ast = parse2(tokens).unwrap();
        let res = retrieve_node_from_position(&ast, Position::new(8, 18));
        println!("{:?}", res);
        if let Some(node) = res {
            match &node {
                FoundNode::StructDefPropertyName(contract, var, ident) => {
                    assert_eq!(contract.is_none(), false);
                    if let Some(name) = ident {
                        assert_eq!(name.to_string(), "storedData1");
                    } else {
                        panic!()
                    }
                }
                _ => {
                    panic!()
                }
            }

        } else {
            panic!()
        }
    }

    #[test]
    fn test_retrieve_file_struct_prop() {
        let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        path.push("tests");
        path.push("files");
        path.push("structs");
        path.push("one.sol");
        let source = fs::read_to_string(path).unwrap();

        let tokens = TokenStream::from_str(source.as_str()).unwrap();
        let ast = parse2(tokens).unwrap();
        let res = retrieve_node_from_position(&ast, Position::new(2, 17));
        println!("{:?}", res);
        if let Some(node) = res {
            match &node {
                FoundNode::StructDefPropertyName(contract, var, ident) => {
                    assert_eq!(contract.is_none(), true);
                    if let Some(name) = ident {
                        assert_eq!(name.to_string(), "storedData1");
                    } else {
                        panic!()
                    }
                }
                _ => {
                    panic!()
                }
            }

        } else {
            panic!()
        }
    }

    #[test]
    fn test_retrieve_contract_enum_def_name() {
        let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        path.push("tests");
        path.push("files");
        path.push("enums");
        path.push("one.sol");
        let source = fs::read_to_string(path).unwrap();

        let tokens = TokenStream::from_str(source.as_str()).unwrap();
        let ast = parse2(tokens).unwrap();
        let res = retrieve_node_from_position(&ast, Position::new(12, 14));
        println!("{:?}", res);
        if let Some(node) = res {
            match &node {
                FoundNode::EnumDefName(contract, ennum, ident) => {
                    assert_eq!(contract.is_none(), false);
                    assert_eq!(ident.to_string(), "another_one");
                }
                _ => {
                    panic!()
                }
            }

        } else {
            panic!()
        }
    }

    #[test]
    fn test_retrieve_contract_enum_def_value() {
        let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        path.push("tests");
        path.push("files");
        path.push("enums");
        path.push("one.sol");
        let source = fs::read_to_string(path).unwrap();

        let tokens = TokenStream::from_str(source.as_str()).unwrap();
        let ast = parse2(tokens).unwrap();
        let res = retrieve_node_from_position(&ast, Position::new(14, 12));
        println!("{:?}", res);
        if let Some(node) = res {
            match &node {
                FoundNode::EnumDefValue(contract, ennum, variant, ident) => {
                    assert_eq!(contract.is_none(), false);
                    assert_eq!(ident.to_string(), "Tuesday");
                }
                _ => {
                    panic!()
                }
            }

        } else {
            panic!()
        }
    }

    #[test]
    fn test_retrieve_file_enum_def_name() {
        let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        path.push("tests");
        path.push("files");
        path.push("enums");
        path.push("one.sol");
        let source = fs::read_to_string(path).unwrap();

        let tokens = TokenStream::from_str(source.as_str()).unwrap();
        let ast = parse2(tokens).unwrap();
        let res = retrieve_node_from_position(&ast, Position::new(1, 8));
        println!("{:?}", res);
        if let Some(node) = res {
            match &node {
                FoundNode::EnumDefName(contract, ennum, ident) => {
                    assert_eq!(contract.is_none(), true);
                    assert_eq!(ident.to_string(), "one");
                }
                _ => {
                    panic!()
                }
            }

        } else {
            panic!()
        }
    }

    #[test]
    fn test_retrieve_file_enum_def_value() {
        let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        path.push("tests");
        path.push("files");
        path.push("enums");
        path.push("one.sol");
        let source = fs::read_to_string(path).unwrap();

        let tokens = TokenStream::from_str(source.as_str()).unwrap();
        let ast = parse2(tokens).unwrap();
        let res = retrieve_node_from_position(&ast, Position::new(3, 8));
        println!("{:?}", res);
        if let Some(node) = res {
            match &node {
                FoundNode::EnumDefValue(contract, ennum, variant, ident) => {
                    assert_eq!(contract.is_none(), true);
                    assert_eq!(ident.to_string(), "Tuesday");
                }
                _ => {
                    panic!()
                }
            }

        } else {
            panic!()
        }
    }

    #[test]
    fn test_retrieve_file_error_def_name() {
        let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        path.push("tests");
        path.push("files");
        path.push("errors");
        path.push("one.sol");
        let source = fs::read_to_string(path).unwrap();

        let tokens = TokenStream::from_str(source.as_str()).unwrap();
        let ast = parse2(tokens).unwrap();
        let res = retrieve_node_from_position(&ast, Position::new(1, 8));
        println!("{:?}", res);
        if let Some(node) = res {
            match &node {
                FoundNode::ErrorDefName(contract, err, ident) => {
                    assert_eq!(contract.is_none(), true);
                    assert_eq!(ident.to_string(), "one");
                }
                _ => {
                    panic!()
                }
            }

        } else {
            panic!()
        }
    }

    #[test]
    fn test_retrieve_contract_error_def_name() {
        let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        path.push("tests");
        path.push("files");
        path.push("errors");
        path.push("one.sol");
        let source = fs::read_to_string(path).unwrap();

        let tokens = TokenStream::from_str(source.as_str()).unwrap();
        let ast = parse2(tokens).unwrap();
        let res = retrieve_node_from_position(&ast, Position::new(4, 16));
        println!("{:?}", res);
        if let Some(node) = res {
            match &node {
                FoundNode::ErrorDefName(contract, err, ident) => {
                    assert_eq!(contract.is_none(), false);
                        assert_eq!(ident.to_string(), "another_one");
                }
                _ => {
                    panic!()
                }
            }

        } else {
            panic!()
        }
    }

    #[test]
    fn test_retrieve_contract_error_def_param() {
        let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        path.push("tests");
        path.push("files");
        path.push("errors");
        path.push("one.sol");
        let source = fs::read_to_string(path).unwrap();

        let tokens = TokenStream::from_str(source.as_str()).unwrap();
        let ast = parse2(tokens).unwrap();
        let res = retrieve_node_from_position(&ast, Position::new(4, 33));
        println!("{:?}", res);
        if let Some(node) = res {
            match &node {
                FoundNode::ErrorDefParameter(contract, err, ident) => {
                    assert_eq!(contract.is_none(), false);
                    if let Some(name) = &ident.name {
                        assert_eq!(name.to_string(), "val1");
                    }
                    else {
                        panic!()
                    }
                }
                _ => {
                    panic!()
                }
            }

        } else {
            panic!()
        }
    }

    #[test]
    fn test_retrieve_file_error_def_param() {
        let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        path.push("tests");
        path.push("files");
        path.push("errors");
        path.push("one.sol");
        let source = fs::read_to_string(path).unwrap();

        let tokens = TokenStream::from_str(source.as_str()).unwrap();
        let ast = parse2(tokens).unwrap();
        let res = retrieve_node_from_position(&ast, Position::new(1, 21));
        println!("{:?}", res);
        if let Some(node) = res {
            match &node {
                FoundNode::ErrorDefParameter(contract, err, ident) => {
                    assert_eq!(contract.is_none(), true);
                    if let Some(name) = &ident.name {
                        assert_eq!(name.to_string(), "val1");
                    }
                    else {
                        panic!()
                    }
                }
                _ => {
                    panic!()
                }
            }

        } else {
            panic!()
        }
    }

    #[test]
    fn test_retrieve_event_def_name() {
        let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        path.push("tests");
        path.push("files");
        path.push("event");
        path.push("one.sol");
        let source = fs::read_to_string(path).unwrap();

        let tokens = TokenStream::from_str(source.as_str()).unwrap();
        let ast = parse2(tokens).unwrap();
        let res = retrieve_node_from_position(&ast, Position::new(4, 16));
        println!("{:?}", res);
        if let Some(node) = res {
            match &node {
                FoundNode::EventDefName(contract, err, ident) => {
                    assert_eq!(ident.to_string(), "another_one");
                }
                _ => {
                    panic!()
                }
            }

        } else {
            panic!()
        }
    }

    #[test]
    fn test_retrieve_event_def_param() {
        let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        path.push("tests");
        path.push("files");
        path.push("event");
        path.push("one.sol");
        let source = fs::read_to_string(path).unwrap();

        let tokens = TokenStream::from_str(source.as_str()).unwrap();
        let ast = parse2(tokens).unwrap();
        let res = retrieve_node_from_position(&ast, Position::new(4, 32));
        println!("{:?}", res);
        if let Some(node) = res {
            match &node {
                FoundNode::EventDefParameter(contract, err, ident) => {
                    if let Some(name) = &ident.name {
                        assert_eq!(name.to_string(), "val1");
                    } else {
                      panic!();
                    }
                }
                _ => {
                    panic!()
                }
            }

        } else {
            panic!()
        }
    }


    #[test]
    fn test_retrieve_prop_usage_on_assign() {
        let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        path.push("tests");
        path.push("files");
        path.push("contracts");
        path.push("two.sol");
        let source = fs::read_to_string(path).unwrap();

        let tokens = TokenStream::from_str(source.as_str()).unwrap();
        let ast = parse2(tokens).unwrap();
        let res = retrieve_node_from_position(&ast, Position::new(5, 14));
        println!("{:?}", res);
        if let Some(node) = res {
            match &node {
                FoundNode::IdentUsageName(contract, func, expr, ident) => {
                    assert_eq!(ident.to_string(), "storedData");
                }
                _ => {
                    panic!()
                }
            }

        } else {
            panic!()
        }
    }

    #[test]
    fn test_retrieve_prop_usage_on_rturn() {
        let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        path.push("tests");
        path.push("files");
        path.push("contracts");
        path.push("two.sol");
        let source = fs::read_to_string(path).unwrap();

        let tokens = TokenStream::from_str(source.as_str()).unwrap();
        let ast = parse2(tokens).unwrap();
        let res = retrieve_node_from_position(&ast, Position::new(9, 22));
        println!("{:?}", res);
        if let Some(node) = res {
            match &node {
                FoundNode::IdentUsageName(contract, func, expr, ident) => {
                    assert_eq!(ident.to_string(), "storedData");
                }
                _ => {
                    panic!()
                }
            }

        } else {
            panic!()
        }
    }

    #[test]
    fn test_retrieve_var_usage_on_assign() {
        let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        path.push("tests");
        path.push("files");
        path.push("contracts");
        path.push("two.sol");
        let source = fs::read_to_string(path).unwrap();

        let tokens = TokenStream::from_str(source.as_str()).unwrap();
        let ast = parse2(tokens).unwrap();
        let res = retrieve_node_from_position(&ast, Position::new(5, 22));
        println!("{:?}", res);
        if let Some(node) = res {
            match &node {
                FoundNode::IdentUsageName(contract, func, expr, ident) => {
                    assert_eq!(ident.to_string(), "x");
                }
                _ => {
                    panic!()
                }
            }

        } else {
            panic!()
        }
    }

}
