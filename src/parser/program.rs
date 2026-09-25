//! Top-level program parser for MiniC.
//!
//! # Overview
//!
//! Exposes one public function:
//!
//! * [`program`] — parses a complete MiniC program as a sequence of zero or
//!   more declarations and returns an [`UncheckedProgram`].
//!
//! A declaration is either an algebraic data type (`type Name = ...;`) or a
//! function. Top-level statements are not allowed. The type checker then
//! verifies that a `main` function exists.
//!
//! # Design Decisions
//!
//! ## Declarations keep source order, split into two lists
//!
//! `Program` stores types and functions in separate vectors. The parser
//! still walks the file once, in order, and pushes each declaration into
//! the matching list, so the relative order inside each list matches the
//! source file.
//!
//! ## Trailing whitespace is consumed
//!
//! `all_consuming(program)` is the CLI entry point. A final `multispace0`
//! eats the newline most source files end with; without it, that newline
//! would be reported as unparsed input.

use crate::ir::ast::{AdtDecl, Program, UncheckedFunDecl, UncheckedProgram};
use crate::parser::functions::fun_decl;
use crate::parser::types::adt_decl;
use nom::{
    branch::alt,
    character::complete::multispace0,
    combinator::map,
    multi::many0,
    sequence::{preceded, terminated},
    IResult,
};

enum Decl {
    Adt(AdtDecl),
    Fun(UncheckedFunDecl),
}

fn decl(input: &str) -> IResult<&str, Decl> {
    alt((map(adt_decl, Decl::Adt), map(fun_decl, Decl::Fun)))(input)
}

/// Parse a complete MiniC program: zero or more type or function declarations.
/// Execution starts at the `main` function (validated by the type checker).
pub fn program(input: &str) -> IResult<&str, UncheckedProgram> {
    map(
        terminated(many0(preceded(multispace0, decl)), multispace0),
        |decls| {
            let mut types = Vec::new();
            let mut functions = Vec::new();
            for d in decls {
                match d {
                    Decl::Adt(a) => types.push(a),
                    Decl::Fun(f) => functions.push(f),
                }
            }
            Program { types, functions }
        },
    )(input)
}
