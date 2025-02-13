use std::collections::BTreeMap;

use crate::{
    BulkString, RespArray, RespError, RespFrame, RespMap, RespNull, RespNullArray,
    RespNullBulkString, SimpleError, SimpleString,
};
use winnow::ascii::{digit1, float};
use winnow::combinator::{alt, dispatch, fail, opt, preceded, terminated};
use winnow::error::ContextError;
use winnow::token::{take, take_until};
use winnow::{token::any, Parser};

const CRLF: &str = "\r\n";

pub fn parse_frame_length(input: &[u8]) -> Result<usize, RespError> {
    let target = &mut (&*input);
    let ret = parse_frame_len(target);
    match ret {
        Ok(_) => {
            // calculate the distance between target and input
            let start = input.as_ptr() as usize;
            let end = (*target).as_ptr() as usize;
            let len = end - start;
            Ok(len)
        }
        Err(_) => Err(RespError::NotComplete),
    }
}

fn parse_frame_len(input: &mut &[u8]) -> winnow::Result<()> {
    let mut simple_parser = terminated(take_until(0.., CRLF), CRLF).value(());
    dispatch! {any;
      b'+' => simple_parser,
      b'-' => simple_parser,
      b':' => simple_parser,
      b'$' => bulk_string_len,
      b'*' => array_len,
      b'_' => simple_parser,
      b'#' => simple_parser,
      b',' => simple_parser,
      b'%' => map_len,
      _v => fail::<_,_,_>
    }
    .parse_next(input)
}

pub fn parse_frame(input: &mut &[u8]) -> winnow::Result<RespFrame> {
    // frame type hash been parsed
    dispatch! {any;
      b'+' => simple_string.map(RespFrame::SimpleString),
      b'-' => error.map(RespFrame::Error),
      b':' => integer.map(RespFrame::Integer),
      b'$' => alt((null_bulk_string.map(RespFrame::NullBulkString), bulk_string.map(RespFrame::BulkString))),
      b'*' => alt((null_array.map(RespFrame::NullArray), array.map(RespFrame::Array))),
      b'_' => null.map(RespFrame::Null),
      b'#' => boolean.map(RespFrame::Boolean),
      b',' => double.map(RespFrame::Double),
      b'%' => map.map(RespFrame::Map),
      _v => fail::<_,_,_>
    }
    .parse_next(input)
}

// - simple string: "OK\r\n"
fn simple_string(input: &mut &[u8]) -> winnow::Result<SimpleString> {
    parse_string(input).map(SimpleString)
}

// - error: "-ERR unknown command 'foobar'\r\n"
fn error(input: &mut &[u8]) -> winnow::Result<SimpleError> {
    parse_string(input).map(SimpleError)
}

// - integer: ":1000\r\n"
fn integer(input: &mut &[u8]) -> winnow::Result<i64> {
    let sign = opt(alt(('+', '-'))).parse_next(input)?.unwrap_or('+');
    let sign = if sign == '-' { -1 } else { 1 };
    let v: i64 = terminated(digit1.parse_to(), CRLF).parse_next(input)?;
    Ok(sign * v)
}

// - null bulk string: "$-1\r\n"
fn null_bulk_string(input: &mut &[u8]) -> winnow::Result<RespNullBulkString> {
    "-1\r\n".value(RespNullBulkString).parse_next(input)
}

// - bulk string: "$6\r\nfoobar\r\n"
fn bulk_string(input: &mut &[u8]) -> winnow::Result<BulkString> {
    let len: i64 = integer(input)?;
    match len {
        0 => return Ok(BulkString::new(vec![])),
        -1 => return Err(err("bulk string length must be non-negative")),
        _ => (),
    }
    let data = terminated(take(len as usize), CRLF).parse_next(input)?;
    Ok(BulkString::new(data))
}

fn bulk_string_len(input: &mut &[u8]) -> winnow::Result<()> {
    let len = integer.parse_next(input)?;
    if len == 0 || len == -1 {
        return Ok(());
    } else if len < -1 {
        return Err(err("bulk string length must be non-negative"));
    }
    terminated(take(len as usize), CRLF)
        .value(())
        .parse_next(input)
}

// - null array: "*-1\r\n"
fn null_array(input: &mut &[u8]) -> winnow::Result<RespNullArray> {
    "-1\r\n".value(RespNullArray).parse_next(input)
}

// - array: "*2\r\n$3\r\nfoo\r\n$3\r\nbar\r\n"
fn array(input: &mut &[u8]) -> winnow::Result<RespArray> {
    let len: i64 = integer(input)?;
    match len {
        0 => return Ok(RespArray::new(vec![])),
        -1 => return Err(err("array length must be non-negative")),
        _ => (),
    }
    let len = len as usize;
    let mut frames = Vec::with_capacity(len);
    for _ in 0..len {
        let frame = parse_frame(input)?;
        frames.push(frame);
    }
    Ok(RespArray::new(frames))
}

fn array_len(input: &mut &[u8]) -> winnow::Result<()> {
    let len = integer.parse_next(input)?;
    if len == 0 || len == -1 {
        return Ok(());
    } else if len < -1 {
        return Err(err("array length must be non-negative"));
    }
    for _ in 0..len {
        parse_frame_len(input)?;
    }
    Ok(())
}

// - boolean: "#t\r\n"
fn boolean(input: &mut &[u8]) -> winnow::Result<bool> {
    let b = terminated(alt(('t', 'f')), CRLF).parse_next(input)?;
    Ok(b == 't')
}

// - float: ",3.14\r\n"
fn double(input: &mut &[u8]) -> winnow::Result<f64> {
    terminated(float, CRLF).parse_next(input)
}

// my understanding of map len is incorrect: https://redis.io/docs/latest/develop/reference/protocol-spec/#maps
// - map: "%1\r\n+foo\r\n-bar\r\n"
fn map(input: &mut &[u8]) -> winnow::Result<RespMap> {
    let len: i64 = integer.parse_next(input)?;
    if len <= 0 {
        return Err(err("map length must be non-negative"));
    }
    let mut map = BTreeMap::new();
    for _ in 0..len {
        let key = preceded('+', parse_string).parse_next(input)?;
        let value = parse_frame(input)?;
        map.insert(key, value);
    }
    Ok(RespMap(map))
}

fn map_len(input: &mut &[u8]) -> winnow::Result<()> {
    let len = integer.parse_next(input)?;
    if len <= 0 {
        return Err(err("map length must be non-negative"));
    }
    for _ in 0..len {
        terminated(take_until(0.., CRLF), CRLF)
            .value(())
            .parse_next(input)?;
        parse_frame_len(input)?;
    }
    Ok(())
}

// - null: "_\r\n"
fn null(input: &mut &[u8]) -> winnow::Result<RespNull> {
    CRLF.value(RespNull).parse_next(input)
}

fn parse_string(input: &mut &[u8]) -> winnow::Result<String> {
    terminated(take_until(0.., CRLF), CRLF)
        .map(|s: &[u8]| String::from_utf8_lossy(s).into_owned())
        .parse_next(input)
}

fn err(_s: impl Into<String>) -> ContextError {
    ContextError::default()
}
