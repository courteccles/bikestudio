//! Unit newtypes, display formatting and input parsing.
//!
//! Everything is stored and solved in millimetres and degrees (S-UNITS U1).
//! Inches exist only at the edges: what a field shows and what the user types.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Millimetres per inch, exactly (U5).
pub const MM_PER_INCH: f64 = 25.4;

/// Deepest parenthesis nesting the parser accepts, so hostile input can't
/// exhaust the stack.
const MAX_NESTING: usize = 32;

/// A length in millimetres.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Default, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Millimeters(pub f64);

/// An angle in degrees.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Default, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Degrees(pub f64);

/// The unit lengths are shown and typed in. An app preference, never project data (U3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LengthUnit {
    #[default]
    Millimeters,
    Inches,
}

impl LengthUnit {
    /// The field suffix: `mm` or `in`.
    pub fn label(self) -> &'static str {
        match self {
            LengthUnit::Millimeters => "mm",
            LengthUnit::Inches => "in",
        }
    }

    /// Decimal places shown (U6).
    pub fn decimals(self) -> usize {
        match self {
            LengthUnit::Millimeters => 1,
            LengthUnit::Inches => 3,
        }
    }

    fn mm_per_unit(self) -> f64 {
        match self {
            LengthUnit::Millimeters => 1.0,
            LengthUnit::Inches => MM_PER_INCH,
        }
    }
}

/// Suspension travel is always shown and exported in millimetres, whatever the
/// display or export unit (U12).
pub const TRAVEL_UNIT: LengthUnit = LengthUnit::Millimeters;

/// The field suffix for angles.
pub const ANGLE_LABEL: &str = "°";

/// Decimal places shown for angles (U6).
pub const ANGLE_DECIMALS: usize = 1;

impl Millimeters {
    /// This length expressed in `unit`.
    pub fn to_unit(self, unit: LengthUnit) -> f64 {
        self.0 / unit.mm_per_unit()
    }

    /// A length given in `unit`, stored as millimetres.
    pub fn from_unit(value: f64, unit: LengthUnit) -> Millimeters {
        Millimeters(value * unit.mm_per_unit())
    }
}

/// Formats a length for display in `unit`, without the unit label.
pub fn format_length(value: Millimeters, unit: LengthUnit) -> String {
    format_fixed(value.to_unit(unit), unit.decimals())
}

/// Formats an angle for display, without the unit label.
pub fn format_angle(value: Degrees) -> String {
    format_fixed(value.0, ANGLE_DECIMALS)
}

fn format_fixed(value: f64, decimals: usize) -> String {
    let text = format!("{value:.decimals$}");
    // Never show "-0.0".
    if text.starts_with('-') && text[1..].chars().all(|c| c == '0' || c == '.') {
        text[1..].to_string()
    } else {
        text
    }
}

/// Why a field's text couldn't be turned into a value. Messages are shown to the user.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ParseError {
    #[error("Enter a value")]
    Empty,
    #[error("Unknown unit '{0}'")]
    UnknownUnit(String),
    #[error("Angles are in degrees; '{0}' is a length unit")]
    LengthUnitOnAngle(String),
    #[error("Fractions aren't supported; type a decimal such as 1.125")]
    Fraction,
    #[error("Unexpected '{0}'")]
    Unexpected(String),
    #[error("The expression ends too early")]
    Incomplete,
    #[error("Missing ')'")]
    UnclosedParenthesis,
    #[error("Too many nested parentheses")]
    TooDeep,
    #[error("Can't divide by zero")]
    DivideByZero,
    #[error("The result is too large")]
    NotFinite,
}

/// Parses a length field (U8, U9).
///
/// `text` is a number or expression in `display_unit`, optionally ending in a
/// unit suffix (`mm`, `in` or `"`) that overrides it. A leading `+` or `-`
/// nudges `current` when there is one.
pub fn parse_length(
    text: &str,
    display_unit: LengthUnit,
    current: Option<Millimeters>,
) -> Result<Millimeters, ParseError> {
    let (expr, suffix) = split_suffix(text)?;
    let unit = match suffix {
        None => display_unit,
        Some(s) => length_unit_for(&s).ok_or(ParseError::UnknownUnit(s))?,
    };
    let current = current.map(|c| c.to_unit(unit));
    let value = evaluate(expr, current)?;
    finite(Millimeters::from_unit(value, unit).0).map(Millimeters)
}

/// Parses an angle field. Accepts `°` or `deg` as a suffix; length units are an error.
pub fn parse_angle(text: &str, current: Option<Degrees>) -> Result<Degrees, ParseError> {
    let (expr, suffix) = split_suffix(text)?;
    if let Some(s) = suffix {
        if length_unit_for(&s).is_some() {
            return Err(ParseError::LengthUnitOnAngle(s));
        }
        if !matches!(s.as_str(), "°" | "deg") {
            return Err(ParseError::UnknownUnit(s));
        }
    }
    evaluate(expr, current.map(|c| c.0)).map(Degrees)
}

/// Commits a length field without drift (U7).
///
/// If the text is unchanged from what was displayed, `current` comes back
/// untouched. Otherwise the text is parsed. Empty text clears the field.
pub fn commit_length(
    text: &str,
    displayed_text: &str,
    current: Option<Millimeters>,
    unit: LengthUnit,
) -> Result<Option<Millimeters>, ParseError> {
    if text.trim() == displayed_text.trim() {
        return Ok(current);
    }
    if text.trim().is_empty() {
        return Ok(None);
    }
    parse_length(text, unit, current).map(Some)
}

/// Commits an angle field without drift (U7).
pub fn commit_angle(
    text: &str,
    displayed_text: &str,
    current: Option<Degrees>,
) -> Result<Option<Degrees>, ParseError> {
    if text.trim() == displayed_text.trim() {
        return Ok(current);
    }
    if text.trim().is_empty() {
        return Ok(None);
    }
    parse_angle(text, current).map(Some)
}

fn length_unit_for(suffix: &str) -> Option<LengthUnit> {
    match suffix {
        "mm" => Some(LengthUnit::Millimeters),
        "in" | "\"" => Some(LengthUnit::Inches),
        _ => None,
    }
}

/// Splits a trailing unit suffix off the expression.
fn split_suffix(text: &str) -> Result<(&str, Option<String>), ParseError> {
    let text = text.trim();
    if text.is_empty() {
        return Err(ParseError::Empty);
    }
    let is_suffix_char = |c: char| c.is_alphabetic() || c == '"' || c == '°';
    let expr = text.trim_end_matches(is_suffix_char);
    if expr.len() == text.len() {
        return Ok((text, None));
    }
    let suffix = text[expr.len()..].to_lowercase();
    let expr = expr.trim_end();
    if expr.is_empty() {
        return Err(ParseError::Unexpected(suffix));
    }
    Ok((expr, Some(suffix)))
}

fn finite(value: f64) -> Result<f64, ParseError> {
    if value.is_finite() {
        Ok(value)
    } else {
        Err(ParseError::NotFinite)
    }
}

/// Evaluates an expression. A leading sign with a `current` value is a nudge.
fn evaluate(expr: &str, current: Option<f64>) -> Result<f64, ParseError> {
    let tokens = tokenize(expr)?;
    let nudge = current.filter(|_| matches!(tokens.first(), Some(Token::Op('+' | '-'))));
    let mut parser = Parser {
        tokens: &tokens,
        pos: 0,
        depth: 0,
    };
    let value = parser.expression()?;
    if let Some(token) = parser.tokens.get(parser.pos) {
        return Err(match token {
            Token::Close => ParseError::Unexpected(")".into()),
            Token::Number(_) => ParseError::Fraction,
            Token::Op(c) => ParseError::Unexpected(c.to_string()),
            Token::Open => ParseError::Unexpected("(".into()),
        });
    }
    finite(nudge.map_or(value, |c| c + value))
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Token {
    Number(f64),
    Op(char),
    Open,
    Close,
}

fn tokenize(expr: &str) -> Result<Vec<Token>, ParseError> {
    let mut tokens = Vec::new();
    let mut chars = expr.char_indices().peekable();
    while let Some((start, c)) = chars.next() {
        match c {
            c if c.is_whitespace() => {}
            '+' | '-' | '*' | '/' => tokens.push(Token::Op(c)),
            '(' => tokens.push(Token::Open),
            ')' => tokens.push(Token::Close),
            c if c.is_ascii_digit() || c == '.' => {
                let mut end = start + c.len_utf8();
                while let Some(&(i, d)) = chars.peek() {
                    if d.is_ascii_digit() || d == '.' {
                        end = i + d.len_utf8();
                        chars.next();
                    } else {
                        break;
                    }
                }
                let text = &expr[start..end];
                let value = text
                    .parse::<f64>()
                    .map_err(|_| ParseError::Unexpected(text.to_string()))?;
                tokens.push(Token::Number(value));
            }
            other => return Err(ParseError::Unexpected(other.to_string())),
        }
    }
    Ok(tokens)
}

struct Parser<'a> {
    tokens: &'a [Token],
    pos: usize,
    depth: usize,
}

impl Parser<'_> {
    fn peek(&self) -> Option<Token> {
        self.tokens.get(self.pos).copied()
    }

    // expression = term (('+' | '-') term)*
    fn expression(&mut self) -> Result<f64, ParseError> {
        let mut value = self.term()?;
        while let Some(Token::Op(op @ ('+' | '-'))) = self.peek() {
            self.pos += 1;
            let rhs = self.term()?;
            value = if op == '+' { value + rhs } else { value - rhs };
        }
        Ok(value)
    }

    // term = factor (('*' | '/') factor)*
    fn term(&mut self) -> Result<f64, ParseError> {
        let mut value = self.factor()?;
        while let Some(Token::Op(op @ ('*' | '/'))) = self.peek() {
            self.pos += 1;
            let rhs = self.factor()?;
            if op == '/' {
                if rhs == 0.0 {
                    return Err(ParseError::DivideByZero);
                }
                value /= rhs;
            } else {
                value *= rhs;
            }
        }
        Ok(value)
    }

    // factor = ('+' | '-') factor | number | '(' expression ')'
    fn factor(&mut self) -> Result<f64, ParseError> {
        match self.peek() {
            None => Err(ParseError::Incomplete),
            Some(Token::Op(op @ ('+' | '-'))) => {
                self.pos += 1;
                self.nest(|p| p.factor())
                    .map(|v| if op == '-' { -v } else { v })
            }
            Some(Token::Number(v)) => {
                self.pos += 1;
                Ok(v)
            }
            Some(Token::Open) => {
                self.pos += 1;
                let value = self.nest(|p| p.expression())?;
                match self.peek() {
                    Some(Token::Close) => {
                        self.pos += 1;
                        Ok(value)
                    }
                    _ => Err(ParseError::UnclosedParenthesis),
                }
            }
            Some(Token::Op(c)) => Err(ParseError::Unexpected(c.to_string())),
            Some(Token::Close) => Err(ParseError::Unexpected(")".into())),
        }
    }

    fn nest(
        &mut self,
        f: impl FnOnce(&mut Self) -> Result<f64, ParseError>,
    ) -> Result<f64, ParseError> {
        if self.depth >= MAX_NESTING {
            return Err(ParseError::TooDeep);
        }
        self.depth += 1;
        let result = f(self);
        self.depth -= 1;
        result
    }
}
