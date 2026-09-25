//! Algebraic data type declaration parser for MiniC.
//!
//! # Overview
//!
//! Exposes two public functions:
//!
//! * [`variant`] — one variant, with or without a positional payload:
//!   `Red` or `Rectangle(int, int)`.
//! * [`adt_decl`] — a full declaration:
//!   `type Name = Variant | Variant(...) | ...;`.
//!
//! # Design Decisions
//!
//! ## Positional payload, not named fields
//!
//! A variant's payload is a comma-separated list of `type_name`s. There
//! are no field names, so the existing `param` parser (which produces
//! `(name, type)`) is not reused.
//!
//! ## Duplicate variant names fail here
//!
//! `type Dup = A | A(int);` is rejected by the parser (`verify` over the
//! variant list), not deferred to a later semantic pass. The grammar in
//! `docs/12-sintaxe-adt.md` lists this as a parse error.

use std::collections::HashSet;

use crate::ir::ast::{AdtDecl, VariantDecl};
use crate::parser::functions::type_name;
use crate::parser::identifiers::identifier;
use nom::{
    bytes::complete::tag,
    character::complete::{char, multispace0, multispace1},
    combinator::{opt, verify},
    multi::{separated_list0, separated_list1},
    sequence::{delimited, preceded},
    IResult,
};

/// Parse one variant: `Name` or `Name(type_name, type_name, ...)`.
pub fn variant(input: &str) -> IResult<&str, VariantDecl> {
    let (rest, name) = preceded(multispace0, identifier)(input)?;
    let (rest, fields) = opt(delimited(
        preceded(multispace0, char('(')),
        separated_list0(preceded(multispace0, char(',')), type_name),
        preceded(multispace0, char(')')),
    ))(rest)?;
    Ok((
        rest,
        VariantDecl {
            name: name.to_string(),
            fields: fields.unwrap_or_default(),
        },
    ))
}

/// Parse an ADT declaration: `"type" ident "=" variant { "|" variant } ";"`.
/// Fails if the declaration has no variants, a trailing comma in a payload,
/// a missing `;`, or two variants with the same name.
pub fn adt_decl(input: &str) -> IResult<&str, AdtDecl> {
    let (rest, _) = preceded(multispace0, tag("type"))(input)?;
    let (rest, name) = preceded(multispace1, identifier)(rest)?;
    let (rest, _) = preceded(multispace0, char('='))(rest)?;
    let (rest, variants) = verify(
        separated_list1(preceded(multispace0, char('|')), variant),
        has_unique_names,
    )(rest)?;
    let (rest, _) = preceded(multispace0, char(';'))(rest)?;
    Ok((
        rest,
        AdtDecl {
            name: name.to_string(),
            variants,
        },
    ))
}

fn has_unique_names(variants: &[VariantDecl]) -> bool {
    let mut seen = HashSet::new();
    variants.iter().all(|v| seen.insert(v.name.as_str()))
}
