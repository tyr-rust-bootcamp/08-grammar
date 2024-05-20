use anyhow::{anyhow, Result};
use std::{collections::HashMap, fmt};
use winnow::{
    ascii::{digit1, multispace0, Caseless},
    combinator::{alt, delimited, opt, separated, separated_pair, trace},
    error::{ContextError, ErrMode, ParserError},
    prelude::*,
    stream::{AsBStr, AsChar, Compare, FindSlice, ParseSlice, Stream, StreamIsPartial},
    token::take_until,
};

#[derive(Debug, Clone, PartialEq)]
enum Num {
    Int(i64),
    Float(f64),
}

#[allow(unused)]
#[derive(Debug, Clone, PartialEq)]
enum JsonValue {
    Null,
    Bool(bool),
    Number(Num),
    String(String),
    Array(Vec<JsonValue>),
    Object(HashMap<String, JsonValue>),
}

fn main() -> Result<()> {
    let s = r#"{
        "name": "John Doe",
        "age": 30,
        "is_student": false,
        "marks": [90.0, -80.0, 85.1],
        "address": {
            "city": "New York",
            "zip": 10001
        }
    }"#;

    let input = &mut (&*s);
    let v = parse_json(input)?;
    println!("{:#?}", v);
    Ok(())
}

fn parse_json(input: &str) -> Result<JsonValue> {
    let input = &mut (&*input);
    parse_value(input).map_err(|e: ErrMode<ContextError>| anyhow!("Failed to parse JSON: {:?}", e))
}
pub fn sep_with_space<Input, Output, Error, ParseNext>(
    mut parser: ParseNext,
) -> impl Parser<Input, (), Error>
where
    Input: Stream + StreamIsPartial,
    <Input as Stream>::Token: AsChar + Clone,
    Error: ParserError<Input>,
    ParseNext: Parser<Input, Output, Error>,
{
    trace("sep_with_space", move |input: &mut Input| {
        let _ = multispace0.parse_next(input)?;
        parser.parse_next(input)?;
        multispace0.parse_next(input)?;
        Ok(())
    })
}

fn parse_null<Input, Error>(input: &mut Input) -> PResult<(), Error>
where
    Input: StreamIsPartial + Stream + Compare<&'static str>,
    Error: ParserError<Input>,
{
    "null".value(()).parse_next(input)
}

fn parse_bool<Input, Error>(input: &mut Input) -> PResult<bool, Error>
where
    Input: StreamIsPartial + Stream + Compare<&'static str>,
    <Input as Stream>::Slice: ParseSlice<bool>,
    Error: ParserError<Input>,
{
    alt(("true", "false")).parse_to().parse_next(input)
}

fn parse_num<Input, Error>(input: &mut Input) -> PResult<Num, Error>
where
    Input: StreamIsPartial
        + Stream
        + Compare<&'static str>
        + Compare<Caseless<&'static str>>
        + Compare<char>
        + AsBStr,
    <Input as Stream>::Slice: ParseSlice<i64>,
    <Input as Stream>::Token: AsChar + Clone,
    <Input as Stream>::IterOffsets: Clone,
    Error: ParserError<Input>,
{
    // process the sign
    let sign = opt("-").map(|s| s.is_some()).parse_next(input)?;
    let num = digit1.parse_to::<i64>().parse_next(input)?;
    let ret: Result<(), ErrMode<ContextError>> = ".".value(()).parse_next(input);
    if ret.is_ok() {
        let frac = digit1.parse_to::<i64>().parse_next(input)?;
        let v = format!("{}.{}", num, frac).parse::<f64>().unwrap();
        Ok(if sign {
            Num::Float(-v as _)
        } else {
            Num::Float(v as _)
        })
    } else {
        Ok(if sign { Num::Int(-num) } else { Num::Int(num) })
    }
}

/// this is too complicated for a single line parser, so we use `float` directly.
// fn parse_number<Input, Error>(input: &mut Input) -> PResult<f64>
// where
//     Input: StreamIsPartial + Stream + Compare<char> + Compare<Caseless<&'static str>>,
//     Input: AsBStr,
//     <Input as Stream>::Slice: ParseSlice<f64>,
//     <Input as Stream>::Token: AsChar + Clone,
//     <Input as Stream>::IterOffsets: Clone,
//     Error: ParserError<Input>,
// {
//     float.parse_next(input)
// }

// json allows quoted strings to have escaped characters, we won't handle that here
fn parse_string<Input, Error>(input: &mut Input) -> PResult<String, Error>
where
    Input: StreamIsPartial + Stream + Compare<char> + FindSlice<char>,
    <Input as Stream>::Token: AsChar + Clone,
    <Input as Stream>::Slice: fmt::Display,
    Error: ParserError<Input>,
{
    let ret = delimited('"', take_until(0.., '"'), '"').parse_next(input)?;
    Ok(ret.to_string())
}

fn parse_array<Input, Error>(input: &mut Input) -> PResult<Vec<JsonValue>, Error>
where
    Input: StreamIsPartial
        + Stream
        + Compare<char>
        + Compare<&'static str>
        + Compare<Caseless<&'static str>>
        + AsBStr
        + FindSlice<char>,
    <Input as Stream>::Token: AsChar + Clone,
    <Input as Stream>::Slice: fmt::Display + ParseSlice<bool> + ParseSlice<i64> + ParseSlice<f64>,
    <Input as Stream>::IterOffsets: Clone,
    Error: ParserError<Input>,
{
    let sep1 = sep_with_space('[');
    let sep2 = sep_with_space(']');
    let sep_comma = sep_with_space(',');
    let parse_values = separated(0.., parse_value, sep_comma);
    delimited(sep1, parse_values, sep2).parse_next(input)
}

fn parse_object<Input, Error>(input: &mut Input) -> PResult<HashMap<String, JsonValue>, Error>
where
    Input: StreamIsPartial
        + Stream
        + Compare<char>
        + Compare<&'static str>
        + Compare<Caseless<&'static str>>
        + AsBStr
        + FindSlice<char>,
    <Input as Stream>::Token: AsChar + Clone,
    <Input as Stream>::Slice: fmt::Display + ParseSlice<bool> + ParseSlice<i64> + ParseSlice<f64>,
    <Input as Stream>::IterOffsets: Clone,
    Error: ParserError<Input>,
{
    let sep1 = sep_with_space('{');
    let sep2 = sep_with_space('}');
    let sep_comma = sep_with_space(',');
    let sep_colon = sep_with_space(':');

    let parse_kv_pair = separated_pair(parse_string, sep_colon, parse_value);
    let parse_kv = separated(1.., parse_kv_pair, sep_comma);
    delimited(sep1, parse_kv, sep2).parse_next(input)
}

fn parse_value<Input, Error>(input: &mut Input) -> PResult<JsonValue, Error>
where
    Input: StreamIsPartial
        + Stream
        + Compare<char>
        + Compare<&'static str>
        + Compare<Caseless<&'static str>>
        + FindSlice<char>,
    Input: AsBStr,
    <Input as Stream>::Token: AsChar + Clone,
    <Input as Stream>::Slice: fmt::Display + ParseSlice<bool> + ParseSlice<i64> + ParseSlice<f64>,
    <Input as Stream>::IterOffsets: Clone,
    Error: ParserError<Input>,
{
    alt((
        parse_null.value(JsonValue::Null),
        parse_bool.map(JsonValue::Bool),
        parse_num.map(JsonValue::Number),
        parse_string.map(JsonValue::String),
        parse_array.map(JsonValue::Array),
        parse_object.map(JsonValue::Object),
    ))
    .parse_next(input)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_null() -> PResult<(), ContextError> {
        let input = "null";
        parse_null(&mut (&*input))?;

        Ok(())
    }

    #[test]
    fn test_parse_bool() -> PResult<(), ContextError> {
        let input = "true";
        let result = parse_bool(&mut (&*input))?;
        assert!(result);

        let input = "false";
        let result = parse_bool(&mut (&*input))?;
        assert!(!result);

        Ok(())
    }

    #[test]
    fn test_parse_num() -> PResult<(), ContextError> {
        let input = "123";
        let result = parse_num(&mut (&*input))?;
        assert_eq!(result, Num::Int(123));

        let input = "-123";
        let result = parse_num(&mut (&*input))?;
        assert_eq!(result, Num::Int(-123));

        let input = "123.456";
        let result = parse_num(&mut (&*input))?;
        assert_eq!(result, Num::Float(123.456));

        let input = "-123.456";
        let result = parse_num(&mut (&*input))?;
        assert_eq!(result, Num::Float(-123.456));

        Ok(())
    }

    #[test]
    fn test_parse_string() -> PResult<(), ContextError> {
        let input = r#""hello""#;
        let result = parse_string(&mut (&*input))?;
        assert_eq!(result, "hello");

        Ok(())
    }

    #[test]
    fn test_parse_array() -> PResult<(), ContextError> {
        let input = r#"[1, 2, 3]"#;
        let result = parse_array(&mut (&*input))?;

        assert_eq!(
            result,
            vec![
                JsonValue::Number(Num::Int(1)),
                JsonValue::Number(Num::Int(2)),
                JsonValue::Number(Num::Int(3))
            ]
        );

        let input = r#"["a", "b", "c"]"#;
        let result = parse_array(&mut (&*input))?;
        assert_eq!(
            result,
            vec![
                JsonValue::String("a".to_string()),
                JsonValue::String("b".to_string()),
                JsonValue::String("c".to_string())
            ]
        );
        Ok(())
    }

    #[test]
    fn test_parse_object() -> PResult<(), ContextError> {
        let input = r#"{"a": 1, "b": 2}"#;
        let result = parse_object(&mut (&*input))?;
        let mut expected = HashMap::new();
        expected.insert("a".to_string(), JsonValue::Number(Num::Int(1)));
        expected.insert("b".to_string(), JsonValue::Number(Num::Int(2)));
        assert_eq!(result, expected);

        let input = r#"{"a": 1, "b": [1, 2, 3]}"#;
        let result = parse_object(&mut (&*input))?;
        let mut expected = HashMap::new();
        expected.insert("a".to_string(), JsonValue::Number(Num::Int(1)));
        expected.insert(
            "b".to_string(),
            JsonValue::Array(vec![
                JsonValue::Number(Num::Int(1)),
                JsonValue::Number(Num::Int(2)),
                JsonValue::Number(Num::Int(3)),
            ]),
        );
        assert_eq!(result, expected);

        Ok(())
    }
}
