//! Phase 1 parser for Orpheus bindings and pattern expressions.

use std::collections::BTreeSet;

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
/// Phase 1 accepts one or more top-level bindings. Bindings are discovered at
/// line starts while the parser is not nested inside parentheses, then each
/// binding body is parsed with the Phase 1 expression grammar.
///
/// # Errors
///
/// Returns [`ParseError`] when the source does not match the Phase 1 grammar
/// or when the parser encounters an internal AST construction failure.
const MAX_PARSE_DEPTH: usize = 128;

/// Parses Phase 1 Orpheus source text into an AST module.
///
/// Phase 1 accepts one or more top-level bindings. Bindings are discovered at
/// line starts while the parser is not nested inside parentheses, then each
/// binding body is parsed with the Phase 1 expression grammar.
///
/// # Errors
///
/// Returns [`ParseError`] when the source does not match the Phase 1 grammar,
/// when the parser encounters an internal AST construction failure, or when
/// the recursion depth limit is exceeded.
pub fn parse_module(source: &str) -> Result<Module, ParseError> {
    check_nesting_depth(source)?;

    let chunks = split_top_level_bindings(source);
    if chunks.is_empty() {
        return parse_single_binding_module(source, 1);
    }

    let mut statements = Vec::new();
    for (start_line, chunk) in chunks {
        let module = parse_single_binding_module(&chunk, start_line)?;
        statements.extend(module.statements);
    }

    Ok(Module { statements })
}

fn parse_single_binding_module(source: &str, start_line: usize) -> Result<Module, ParseError> {
    let padded_source = if start_line <= 1 {
        source.to_owned()
    } else {
        format!("{}{}", "\n".repeat(start_line - 1), source)
    };
    let mut pairs = SyntaxParser::parse(Rule::module, &padded_source)
        .map_err(|error| enrich_parse_error(source, &error))?;
    let module_pair = next_pair(&mut pairs, "module")?;
    build_module(module_pair)
}

fn split_top_level_bindings(source: &str) -> Vec<(usize, String)> {
    let mut bindings = Vec::new();
    let mut current = String::new();
    let mut current_start_line = 1_usize;
    let mut paren_depth = 0_i32;

    for (index, line) in source.lines().enumerate() {
        let line_number = index + 1;
        let trimmed = line.trim();
        if paren_depth == 0 && !current.trim().is_empty() && looks_like_binding(trimmed) {
            bindings.push((current_start_line, std::mem::take(&mut current)));
            current_start_line = line_number;
        } else if current.is_empty() && !trimmed.is_empty() {
            current_start_line = line_number;
        }

        if !current.is_empty() {
            current.push('\n');
        }
        current.push_str(line);
        paren_depth = update_paren_depth(paren_depth, line);
    }

    if !current.trim().is_empty() {
        bindings.push((current_start_line, current));
    }

    bindings
}

fn enrich_parse_error(source: &str, error: &PestError<Rule>) -> ParseError {
    let (line, col) = match error.line_col {
        pest::error::LineColLocation::Pos((l, c))
        | pest::error::LineColLocation::Span((l, c), _) => (l, c),
    };

    let reason = match &error.variant {
        pest::error::ErrorVariant::ParsingError { positives, .. } => {
            if positives.is_empty() {
                "unexpected token".to_owned()
            } else {
                let expected = positives
                    .iter()
                    .map(|r| format!("{r:?}"))
                    .collect::<Vec<_>>()
                    .join(" or ");
                format!("expected {expected}")
            }
        }
        pest::error::ErrorVariant::CustomError { message } => message.clone(),
    };

    if unmatched_open_parens(source) > 0 {
        return ParseError::new(format!(
            "parse error at line {line}, col {col}: missing `)` before end of input"
        ));
    }

    ParseError::new(format!("parse error at line {line}, col {col}: {reason}"))
}

fn looks_like_binding(line: &str) -> bool {
    let Some((header, _expr)) = line.split_once('=') else {
        return false;
    };

    let mut identifiers = header.split_whitespace();
    let Some(first) = identifiers.next() else {
        return false;
    };

    is_identifier(first) && identifiers.all(is_identifier)
}

fn is_identifier(candidate: &str) -> bool {
    let mut chars = candidate.chars();
    let Some(first) = chars.next() else {
        return false;
    };

    (first.is_ascii_alphabetic() || first == '_')
        && chars.all(|character| character.is_ascii_alphanumeric() || character == '_')
}

fn update_paren_depth(current: i32, line: &str) -> i32 {
    let mut depth = current;
    let mut in_string = false;
    let mut escaping = false;

    for character in line.chars() {
        if in_string {
            if escaping {
                escaping = false;
                continue;
            }
            match character {
                '\\' => escaping = true,
                '"' => in_string = false,
                _ => {}
            }
            continue;
        }

        match character {
            '"' => in_string = true,
            '(' => depth = depth.saturating_add(1),
            ')' => depth = depth.saturating_sub(1),
            _ => {}
        }
    }

    depth
}

fn check_nesting_depth(source: &str) -> Result<(), ParseError> {
    let mut parens_depth: usize = 0;
    let mut pipe_depth: usize = 0;
    let mut max_parens_depth: usize = 0;
    let mut max_pipe_depth: usize = 0;
    let mut in_string = false;
    let mut escaping = false;
    let mut last_char = ' ';

    for character in source.chars() {
        if in_string {
            if escaping {
                escaping = false;
                continue;
            }
            match character {
                '\\' => escaping = true,
                '"' => in_string = false,
                _ => {}
            }
        } else {
            match character {
                '"' => in_string = true,
                '(' => {
                    parens_depth += 1;
                    if parens_depth > max_parens_depth {
                        max_parens_depth = parens_depth;
                    }
                }
                ')' => {
                    parens_depth = parens_depth.saturating_sub(1);
                }
                '>' => {
                    if last_char == '|' {
                        pipe_depth += 1;
                        if pipe_depth > max_pipe_depth {
                            max_pipe_depth = pipe_depth;
                        }
                    }
                }
                '\n' | '\r' => {
                    pipe_depth = 0;
                }
                _ => {}
            }
        }
        last_char = character;
    }

    if max_parens_depth > MAX_PARSE_DEPTH || max_pipe_depth > MAX_PARSE_DEPTH {
        return Err(ParseError::new("recursion limit exceeded during parsing"));
    }
    Ok(())
}

fn unmatched_open_parens(source: &str) -> usize {
    let mut count = 0_usize;
    let mut in_string = false;
    let mut escaping = false;

    for character in source.chars() {
        if in_string {
            if escaping {
                escaping = false;
                continue;
            }
            match character {
                '\\' => escaping = true,
                '"' => in_string = false,
                _ => {}
            }
            continue;
        }

        match character {
            '"' => in_string = true,
            '(' => count += 1,
            ')' => count = count.saturating_sub(1),
            _ => {}
        }
    }

    count
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
    let (name, params) = build_binding_head(next_pair(&mut inner, "binding head")?)?;
    let expr = build_pipe_expr(next_pair(&mut inner, "binding expression")?)?;
    Ok(Stmt::Binding { name, params, expr })
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
        Rule::postfix => build_postfix(pair),
        Rule::primary => build_expr(first_inner(pair, "primary expression")?),
        Rule::group => build_group(pair),
        Rule::rest => Ok(Expr::Rest),
        Rule::number => build_number(&pair),
        Rule::string => build_string(&pair),
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

fn build_binding_head(pair: Pair<'_, Rule>) -> Result<(String, Vec<String>), ParseError> {
    let mut identifiers = pair
        .into_inner()
        .filter(|inner| inner.as_rule() == Rule::identifier);

    let name = identifiers
        .next()
        .map(|identifier| identifier.as_str().to_owned())
        .ok_or_else(|| ParseError::new("missing binding name"))?;
    let mut seen = BTreeSet::new();
    let mut params = Vec::new();

    for identifier in identifiers {
        let param = identifier.as_str().to_owned();
        if !seen.insert(param.clone()) {
            let (line, col) = identifier.as_span().start_pos().line_col();
            return Err(ParseError::new(format!(
                "parse error at line {line}, col {col}: duplicate parameter `{param}` in binding `{name}`"
            )));
        }
        params.push(param);
    }

    Ok((name, params))
}

fn build_stack(pair: Pair<'_, Rule>) -> Result<Expr, ParseError> {
    let layers_pair = next_pair(&mut pair.into_inner(), "stack layers")?;
    let layers = layers_pair
        .into_inner()
        .map(build_pipe_expr)
        .collect::<Result<Vec<_>, _>>()?;

    Ok(Expr::Stack(layers))
}

fn build_postfix(pair: Pair<'_, Rule>) -> Result<Expr, ParseError> {
    let mut inner = pair.into_inner();
    let first = next_pair(&mut inner, "postfix callee")?;
    let mut expr = build_expr(first)?;

    for suffix in inner {
        let args = build_call_suffix_args(suffix)?;
        expr = build_call_expr(expr, args)?;
    }

    Ok(expr)
}

fn build_call_suffix_args(pair: Pair<'_, Rule>) -> Result<Vec<Expr>, ParseError> {
    let mut inner = pair.into_inner();
    let Some(args_pair) = inner.next() else {
        return Ok(Vec::new());
    };

    args_pair
        .into_inner()
        .map(build_pipe_expr)
        .collect::<Result<Vec<_>, _>>()
}

fn build_call_expr(callee: Expr, args: Vec<Expr>) -> Result<Expr, ParseError> {
    if let Expr::Ident(callee_name) = &callee {
        match callee_name.as_str() {
            "stream" => return Ok(Expr::Stream(args)),
            "at" => match args.as_slice() {
                [start, pattern] => {
                    return Ok(Expr::At {
                        start: Box::new(start.clone()),
                        pattern: Box::new(pattern.clone()),
                    });
                }
                _ => return Err(ParseError::new("`at` requires exactly two arguments")),
            },
            "meter" => match args.as_slice() {
                [beats, unit, pattern] => {
                    return Ok(Expr::Meter {
                        beats: Box::new(beats.clone()),
                        unit: Box::new(unit.clone()),
                        pattern: Box::new(pattern.clone()),
                    });
                }
                [_, _] => {}
                _ => return Err(ParseError::new("`meter` requires exactly three arguments")),
            },
            "beat" => match args.as_slice() {
                [value] => return Ok(Expr::Beat(Box::new(value.clone()))),
                _ => return Err(ParseError::new("`beat` requires exactly one argument")),
            },
            "section" => match args.as_slice() {
                [pattern, cycles] => {
                    return Ok(Expr::Section {
                        pattern: Box::new(pattern.clone()),
                        cycles: Box::new(cycles.clone()),
                    });
                }
                _ => return Err(ParseError::new("`section` requires exactly two arguments")),
            },
            "seq_sections" => return Ok(Expr::SeqSections(args)),
            _ => {}
        }
    }

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

fn build_string(pair: &Pair<'_, Rule>) -> Result<Expr, ParseError> {
    parse_string_literal(pair.as_str()).map(Expr::String)
}

fn parse_string_literal(literal: &str) -> Result<String, ParseError> {
    let Some(body) = literal
        .strip_prefix('"')
        .and_then(|stripped| stripped.strip_suffix('"'))
    else {
        return Err(ParseError::new("invalid string literal delimiter"));
    };

    let mut value = String::new();
    let mut characters = body.chars();
    while let Some(character) = characters.next() {
        if character != '\\' {
            value.push(character);
            continue;
        }

        let Some(escaped) = characters.next() else {
            return Err(ParseError::new("unterminated string escape"));
        };
        match escaped {
            '"' => value.push('"'),
            '\\' => value.push('\\'),
            'n' => value.push('\n'),
            'r' => value.push('\r'),
            't' => value.push('\t'),
            other => {
                return Err(ParseError::new(format!(
                    "unsupported string escape `\\{other}`"
                )));
            }
        }
    }

    Ok(value)
}

fn collapse_sequence(items: Vec<Expr>, context: &'static str) -> Result<Expr, ParseError> {
    if let Some(expr) = collapse_meter_annotation(&items)? {
        return Ok(expr);
    }

    match items.len() {
        0 => Err(ParseError::new(format!("missing {context} items"))),
        1 => Ok(items
            .into_iter()
            .next()
            .ok_or_else(|| ParseError::new(format!("missing {context} item")))?),
        _ => Ok(Expr::Seq(items)),
    }
}

fn collapse_meter_annotation(items: &[Expr]) -> Result<Option<Expr>, ParseError> {
    let Some((first, rest)) = items.split_first() else {
        return Ok(None);
    };

    let Expr::Call { callee, args } = first else {
        return Ok(None);
    };
    let Expr::Ident(name) = callee.as_ref() else {
        return Ok(None);
    };
    if name != "meter" {
        return Ok(None);
    }

    let [beats, unit] = args.as_slice() else {
        return Ok(None);
    };
    if rest.is_empty() {
        return Err(ParseError::new(
            "`meter` annotation requires a following pattern expression",
        ));
    }

    let pattern = collapse_sequence(rest.to_vec(), "meter annotation")?;
    Ok(Some(Expr::Meter {
        beats: Box::new(beats.clone()),
        unit: Box::new(unit.clone()),
        pattern: Box::new(pattern),
    }))
}
