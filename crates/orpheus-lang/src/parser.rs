//! Phase 1 parser for Orpheus bindings and pattern expressions.

use pest::Parser;
use pest::error::Error as PestError;
use pest::iterators::{Pair, Pairs};
use pest_derive::Parser;

use crate::ast::{Expr, Module, Stmt};
use crate::diagnostics::ParseError;

#[derive(Parser)]
#[grammar = "grammar/orpheus.pest"]
struct SyntaxParser;

/// Parses Phase 1 Orpheus source text into an AST module.
///
/// Phase 1 intentionally accepts a single top-level binding only. Multi-binding
/// separator rules land in a later task once the surface syntax is specified.
///
/// # Errors
///
/// Returns [`ParseError`] when the source does not match the Phase 1 grammar
/// or when the parser encounters an internal AST construction failure.
pub fn parse_module(source: &str) -> Result<Module, ParseError> {
    let mut pairs = SyntaxParser::parse(Rule::module, source)
        .map_err(|error| enrich_parse_error(source, &error))?;
    let module_pair = next_pair(&mut pairs, "module")?;
    build_module(module_pair)
}

fn enrich_parse_error(source: &str, error: &PestError<Rule>) -> ParseError {
    let rendered = error.to_string();

    if let Some((line_number, binding)) = second_top_level_binding(source) {
        return ParseError::new(format!(
            "parse error: Phase 1 accepts a single top-level binding; found another binding at line {line_number}: {binding}"
        ));
    }

    if unmatched_open_parens(source) > 0 {
        return ParseError::new(format!(
            "parse error: missing `)` before end of input; {rendered}"
        ));
    }

    ParseError::new(format!("parse error: {rendered}"))
}

fn second_top_level_binding(source: &str) -> Option<(usize, &str)> {
    source
        .lines()
        .enumerate()
        .filter_map(|(index, line)| {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some((index + 1, trimmed))
            }
        })
        .skip(1)
        .find(|(_, line)| looks_like_binding(line))
}

fn looks_like_binding(line: &str) -> bool {
    let Some((name, _expr)) = line.split_once('=') else {
        return false;
    };

    let candidate = name.trim();
    let mut chars = candidate.chars();
    let Some(first) = chars.next() else {
        return false;
    };

    (first.is_ascii_alphabetic() || first == '_')
        && chars.all(|character| character.is_ascii_alphanumeric() || character == '_')
}

fn unmatched_open_parens(source: &str) -> usize {
    source
        .chars()
        .fold(0_usize, |count, character| match character {
            '(' => count + 1,
            ')' => count.saturating_sub(1),
            _ => count,
        })
}

fn next_pair<'a>(
    pairs: &mut Pairs<'a, Rule>,
    context: &'static str,
) -> Result<Pair<'a, Rule>, ParseError> {
    pairs
        .next()
        .ok_or_else(|| ParseError::new(format!("missing {context}")))
}

fn first_inner<'a>(
    pair: Pair<'a, Rule>,
    context: &'static str,
) -> Result<Pair<'a, Rule>, ParseError> {
    pair.into_inner()
        .next()
        .ok_or_else(|| ParseError::new(format!("missing {context}")))
}

fn build_module(pair: Pair<'_, Rule>) -> Result<Module, ParseError> {
    let statements = pair
        .into_inner()
        .filter(|inner| inner.as_rule() == Rule::binding)
        .map(build_binding)
        .collect::<Result<Vec<_>, _>>()?;

    Ok(Module { statements })
}

fn build_binding(pair: Pair<'_, Rule>) -> Result<Stmt, ParseError> {
    let mut inner = pair.into_inner();
    let name = next_pair(&mut inner, "binding name")?.as_str().to_owned();
    let expr = build_pipe_expr(next_pair(&mut inner, "binding expression")?)?;
    Ok(Stmt::Binding { name, expr })
}

fn build_pipe_expr(pair: Pair<'_, Rule>) -> Result<Expr, ParseError> {
    let mut inner = pair.into_inner();
    let first = build_sequence(next_pair(&mut inner, "pipe lhs")?)?;

    inner.try_fold(first, |lhs, rhs| {
        Ok(Expr::Pipe {
            lhs: Box::new(lhs),
            rhs: Box::new(build_pipe_target(rhs)?),
        })
    })
}

fn build_sequence(pair: Pair<'_, Rule>) -> Result<Expr, ParseError> {
    let items = pair
        .into_inner()
        .map(build_item)
        .collect::<Result<Vec<_>, _>>()?;

    collapse_sequence(items, "sequence")
}

fn build_item(pair: Pair<'_, Rule>) -> Result<Expr, ParseError> {
    build_expr(first_inner(pair, "sequence item")?)
}

fn build_pipe_target(pair: Pair<'_, Rule>) -> Result<Expr, ParseError> {
    build_expr(first_inner(pair, "pipe target")?)
}

fn build_expr(pair: Pair<'_, Rule>) -> Result<Expr, ParseError> {
    match pair.as_rule() {
        Rule::stack => build_stack(pair),
        Rule::call => build_call(pair),
        Rule::group => build_group(pair),
        Rule::rest => Ok(Expr::Rest),
        Rule::number => build_number(&pair),
        Rule::identifier => Ok(Expr::Ident(pair.as_str().to_owned())),
        Rule::pipe_expr => build_pipe_expr(pair),
        Rule::sequence => build_sequence(pair),
        Rule::item => build_item(pair),
        Rule::pipe_target => build_pipe_target(pair),
        other => Err(ParseError::new(format!(
            "unexpected parser rule while building AST: {other:?}"
        ))),
    }
}

fn build_stack(pair: Pair<'_, Rule>) -> Result<Expr, ParseError> {
    let layers_pair = next_pair(&mut pair.into_inner(), "stack layers")?;
    let layers = layers_pair
        .into_inner()
        .map(build_pipe_expr)
        .collect::<Result<Vec<_>, _>>()?;

    Ok(Expr::Stack(layers))
}

fn build_call(pair: Pair<'_, Rule>) -> Result<Expr, ParseError> {
    let mut inner = pair.into_inner();
    let callee = Expr::Ident(next_pair(&mut inner, "call callee")?.as_str().to_owned());
    let args = if let Some(args_pair) = inner.next() {
        args_pair
            .into_inner()
            .map(build_pipe_expr)
            .collect::<Result<Vec<_>, _>>()?
    } else {
        Vec::new()
    };

    Ok(Expr::Call {
        callee: Box::new(callee),
        args,
    })
}

fn build_group(pair: Pair<'_, Rule>) -> Result<Expr, ParseError> {
    let items = pair
        .into_inner()
        .map(build_item)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Expr::Group(items))
}

fn build_number(pair: &Pair<'_, Rule>) -> Result<Expr, ParseError> {
    pair.as_str()
        .parse::<f64>()
        .map(Expr::Number)
        .map_err(|error| {
            ParseError::new(format!(
                "invalid number literal `{}`: {error}",
                pair.as_str()
            ))
        })
}

fn collapse_sequence(items: Vec<Expr>, context: &'static str) -> Result<Expr, ParseError> {
    match items.len() {
        0 => Err(ParseError::new(format!("missing {context} items"))),
        1 => Ok(items
            .into_iter()
            .next()
            .ok_or_else(|| ParseError::new(format!("missing {context} item")))?),
        _ => Ok(Expr::Seq(items)),
    }
}
