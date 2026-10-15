//! Parsers for patterns and `match` statements.

use crate::ir::ast::{MatchArm, Pattern, Statement, StatementD, UncheckedStmt};
use crate::parser::expressions::expression;
use crate::parser::identifiers::identifier;
use crate::parser::statements::block_statement;
use nom::{
    branch::alt,
    bytes::complete::tag,
    character::complete::{char, multispace0},
    combinator::{map, opt, verify},
    multi::{many1, separated_list1},
    sequence::{delimited, preceded},
    IResult,
};

fn upper_ident(input: &str) -> IResult<&str, &str> {
    verify(preceded(multispace0, identifier), |name: &&str| {
        name.chars()
            .next()
            .is_some_and(|character| character.is_uppercase())
    })(input)
}

fn lower_ident(input: &str) -> IResult<&str, &str> {
    verify(preceded(multispace0, identifier), |name: &&str| {
        name.chars()
            .next()
            .is_some_and(|character| character.is_lowercase())
    })(input)
}

fn pattern(input: &str) -> IResult<&str, Pattern> {
    alt((
        map(
            verify(preceded(multispace0, identifier), |name: &&str| {
                *name == "_"
            }),
            |_| Pattern::Wildcard,
        ),
        map(variant_pattern, |(name, fields)| Pattern::Variant {
            name,
            fields,
        }),
        map(lower_ident, |name| Pattern::Bind(name.to_string())),
    ))(input)
}

fn variant_pattern(input: &str) -> IResult<&str, (String, Vec<Pattern>)> {
    let (rest, name) = upper_ident(input)?;
    let (rest, fields) = opt(delimited(
        preceded(multispace0, char('(')),
        separated_list1(preceded(multispace0, char(',')), pattern),
        preceded(multispace0, char(')')),
    ))(rest)?;

    Ok((rest, (name.to_string(), fields.unwrap_or_default())))
}

fn match_arm(input: &str) -> IResult<&str, MatchArm<()>> {
    let (rest, pattern) = preceded(multispace0, pattern)(input)?;
    let (rest, _) = preceded(multispace0, tag("=>"))(rest)?;
    let (rest, body) = preceded(multispace0, block_statement)(rest)?;

    Ok((
        rest,
        MatchArm {
            pattern,
            body: Box::new(body),
        },
    ))
}

/// Parse a `match (expression) { pattern => { statements } ... }` statement.
pub fn match_statement(input: &str) -> IResult<&str, UncheckedStmt> {
    let (rest, _) = preceded(multispace0, tag("match"))(input)?;
    let (rest, scrutinee) = delimited(
        preceded(multispace0, char('(')),
        preceded(multispace0, expression),
        preceded(multispace0, char(')')),
    )(rest)?;
    let (rest, arms) = delimited(
        preceded(multispace0, char('{')),
        many1(preceded(multispace0, match_arm)),
        preceded(multispace0, char('}')),
    )(rest)?;

    Ok((
        rest,
        StatementD {
            stmt: Statement::Match {
                scrutinee: Box::new(scrutinee),
                arms,
            },
            ty: (),
        },
    ))
}

#[cfg(test)]
mod tests {
    use super::{match_statement, pattern};
    use crate::ir::ast::{Expr, Pattern, Statement};
    use crate::parser::statements::statement;
    use nom::combinator::all_consuming;

    #[test]
    fn parses_wildcard_binding_and_variant_patterns() {
        assert_eq!(all_consuming(pattern)("_").unwrap().1, Pattern::Wildcard);
        assert_eq!(
            all_consuming(pattern)("value").unwrap().1,
            Pattern::Bind("value".to_string())
        );
        assert_eq!(
            all_consuming(pattern)("None").unwrap().1,
            Pattern::Variant {
                name: "None".to_string(),
                fields: vec![],
            }
        );
        assert_eq!(
            all_consuming(pattern)("Rectangle(width, height)")
                .unwrap()
                .1,
            Pattern::Variant {
                name: "Rectangle".to_string(),
                fields: vec![
                    Pattern::Bind("width".to_string()),
                    Pattern::Bind("height".to_string()),
                ],
            }
        );
    }

    #[test]
    fn rejects_non_pattern_arguments_and_wrong_case() {
        assert!(all_consuming(pattern)("Some(1 + 1)").is_err());
        assert!(all_consuming(pattern)("some(value)").is_err());
        assert!(all_consuming(pattern)("Some()").is_err());
    }

    #[test]
    fn parses_match_as_a_statement_and_builds_the_ast() {
        let parsed = all_consuming(statement)(
            "match (item) { Some(value) => { print(value); } _ => { print(0); } }",
        )
        .unwrap()
        .1;

        let Statement::Match { scrutinee, arms } = parsed.stmt else {
            panic!("expected a match statement");
        };

        assert_eq!(scrutinee.exp, Expr::Ident("item".to_string()));
        assert_eq!(arms.len(), 2);
        assert_eq!(
            arms[0].pattern,
            Pattern::Variant {
                name: "Some".to_string(),
                fields: vec![Pattern::Bind("value".to_string())],
            }
        );
        let Statement::Block { seq } = &arms[0].body.stmt else {
            panic!("expected match arm body block");
        };
        let Statement::Call { name, .. } = &seq[0].stmt else {
            panic!("expected print call");
        };
        assert_eq!(name, "print");
        assert_eq!(arms[1].pattern, Pattern::Wildcard);
    }

    #[test]
    fn rejects_malformed_match_statements() {
        for source in [
            "match item { _ => 0; }",
            "match (item) { _ { print(0); } }",
            "match (item) { _ => print(0); }",
            "match (item) {}",
        ] {
            assert!(
                all_consuming(match_statement)(source).is_err(),
                "accepted {source}"
            );
        }
    }

    #[test]
    fn match_is_not_an_expression() {
        assert!(
            all_consuming(crate::parser::expressions::expression)("match (item) { _ => {} }")
                .is_err()
        );
    }
}
