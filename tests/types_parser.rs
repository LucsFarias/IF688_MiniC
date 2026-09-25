//! Parser tests for algebraic data type declarations.

use mini_c::ir::ast::Type;
use mini_c::parser::types::{adt_decl, variant};
use nom::combinator::all_consuming;

#[test]
fn test_variant_without_payload() {
    let v = all_consuming(variant)("Red").unwrap().1;
    assert_eq!(v.name, "Red");
    assert!(v.fields.is_empty());
}

#[test]
fn test_variant_one_field() {
    let v = all_consuming(variant)("Circle(int)").unwrap().1;
    assert_eq!(v.name, "Circle");
    assert_eq!(v.fields, vec![Type::Int]);
}

#[test]
fn test_variant_two_fields() {
    let v = all_consuming(variant)("Rectangle(int, int)").unwrap().1;
    assert_eq!(v.name, "Rectangle");
    assert_eq!(v.fields, vec![Type::Int, Type::Int]);
}

#[test]
fn test_adt_color() {
    let decl = all_consuming(adt_decl)("type Color = Red | Green | Blue;")
        .unwrap()
        .1;
    assert_eq!(decl.name, "Color");
    assert_eq!(
        decl.variants
            .iter()
            .map(|v| v.name.as_str())
            .collect::<Vec<_>>(),
        vec!["Red", "Green", "Blue"]
    );
    assert!(decl.variants.iter().all(|v| v.fields.is_empty()));
}

#[test]
fn test_adt_shape() {
    let decl = all_consuming(adt_decl)("type Shape = Circle(int) | Rectangle(int, int);")
        .unwrap()
        .1;
    assert_eq!(decl.name, "Shape");
    assert_eq!(decl.variants[0].name, "Circle");
    assert_eq!(decl.variants[0].fields, vec![Type::Int]);
    assert_eq!(decl.variants[1].name, "Rectangle");
    assert_eq!(decl.variants[1].fields, vec![Type::Int, Type::Int]);
}

#[test]
fn test_adt_option_int() {
    let decl = all_consuming(adt_decl)("type OptionInt = Some(int) | None;")
        .unwrap()
        .1;
    assert_eq!(decl.name, "OptionInt");
    assert_eq!(decl.variants[0].name, "Some");
    assert_eq!(decl.variants[0].fields, vec![Type::Int]);
    assert_eq!(decl.variants[1].name, "None");
    assert!(decl.variants[1].fields.is_empty());
}

#[test]
fn test_adt_without_variants_fails() {
    assert!(all_consuming(adt_decl)("type Vazio = ;").is_err());
}

#[test]
fn test_adt_duplicate_variant_name_fails() {
    assert!(all_consuming(adt_decl)("type Dup = A | A(int);").is_err());
}

#[test]
fn test_adt_trailing_comma_fails() {
    assert!(all_consuming(adt_decl)("type Shape = Circle(int,);").is_err());
}

#[test]
fn test_adt_missing_semicolon_fails() {
    assert!(all_consuming(adt_decl)("type Cor = Vermelho | Verde").is_err());
}
