// Suíte de aceitação para ADTs (tipos algébricos) e constructors.

// Pra rodar só este arquivo: `cargo test --test adt`

use mini_c::ir::ast::{AdtDecl, Expr, Type, VariantDecl};
use mini_c::parser::{adt_decl, expression, program};
use nom::combinator::all_consuming;

// Construct: exemplos canônicos

#[test]
fn construct_with_payload() {
    let (rest, e) = expression("Circle(5)").expect("deveria parsear");
    assert_eq!(rest, "");
    match e.exp {
        Expr::Construct { name, args } => {
            assert_eq!(name, "Circle");
            assert_eq!(args.len(), 1);
        }
        other => panic!("esperava Expr::Construct, veio {other:?}"),
    }
}

#[test]
fn construct_without_payload() {
    let (rest, e) = expression("None").expect("deveria parsear");
    assert_eq!(rest, "");
    match e.exp {
        Expr::Construct { name, args } => {
            assert_eq!(name, "None");
            assert!(args.is_empty());
        }
        other => panic!("esperava Expr::Construct, veio {other:?}"),
    }
}

#[test]
fn uppercase_before_lowercase_call_in_atom_ordering() {
    // Regressão: construct() precisa vir antes de parse_call em atom(),
    // senão isso aqui viraria Expr::Call em vez de Expr::Construct.
    let (_, some) = expression("Some(42)").unwrap();
    assert!(matches!(some.exp, Expr::Construct { .. }));

    // E chamada de função comum (minúscula) continua funcionando normal.
    let (_, call) = expression("soma(1, 2)").unwrap();
    assert!(matches!(call.exp, Expr::Call { .. }));
}

#[test]
fn option_int_declaration_shape() {
    let (rest, decl) = adt_decl("type OptionInt = Some(int) | None;").unwrap();
    assert_eq!(rest, "");
    assert_eq!(
        decl,
        AdtDecl {
            name: "OptionInt".to_string(),
            variants: vec![
                VariantDecl { name: "Some".to_string(), fields: vec![Type::Int] },
                VariantDecl { name: "None".to_string(), fields: vec![] },
            ],
        }
    );
}

// ---------------------------------------------------------------------
// Integração: all_consuming (Pessoa 2, seção 4.3) — lixo no fim do
// arquivo deve ser rejeitado, não silenciosamente ignorado.
// ---------------------------------------------------------------------

#[test]
fn garbage_after_last_declaration_is_rejected() {
    let source = "type Cor = Vermelho | Verde;\nisso nao e um programa valido";
    let result = all_consuming(program)(source);
    assert!(result.is_err(), "entrada com lixo no fim deveria falhar");
}

// ---------------------------------------------------------------------
// ATENÇÃO: "None(1)" e "Rectangle(1)" NÃO são rejeitados pelo parser hoje.
// Ao ver "None(1)" isolado, o parser não sabe quantos campos "None" deveria
// ter. Isso só dá pra checar comparando com a declaração do ADT, o que é
// trabalho do verificador de tipos (fora de escopo da entrega 1). Por
// isso NÃO tem teste aqui dizendo que "None(1)" falha, ele não falha, e
// está certo que não falhe agora.
// ---------------------------------------------------------------------

// ---------------------------------------------------------------------
// Bloqueados até a terminar match_expr/pattern(). Com
// #[ignore] pra não quebrar o `cargo test` de ninguém nesse meio tempo.
// Quando atom() tiver match_expr: tira o #[ignore] e preenche o corpo.
// ---------------------------------------------------------------------

#[test]
#[ignore = "bloqueado: match_expr ainda não existe em atom() (Pessoa 3)"]
fn match_with_some_and_wildcard_shape() {
    // match (a) { Some(v) => v; _ => 0; } deve virar Expr::Match com 2
    // braços: Pattern::Variant { name: "Some", fields: [Pattern::Bind("v")] }
    // e Pattern::Wildcard.
    todo!("preencher quando match_expr estiver pronto")
}

#[test]
#[ignore = "bloqueado: match_expr ainda não existe em atom() (Pessoa 3)"]
fn match_arm_without_arrow_is_rejected() {
    // match (s) { Circle(r) r * r; } — braço sem "=>" deve falhar.
    todo!("preencher quando match_expr estiver pronto")
}