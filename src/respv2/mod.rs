mod parser;

use crate::{RespError, RespFrame};
use bytes::BytesMut;
use parser::{parse_frame, parse_frame_length};

#[allow(unused)]
pub trait RespDecodeV2: Sized {
    fn decode(buf: &mut BytesMut) -> Result<Self, RespError>;
    fn expect_length(buf: &[u8]) -> Result<usize, RespError>;
}

impl RespDecodeV2 for RespFrame {
    fn decode(buf: &mut BytesMut) -> Result<Self, RespError> {
        let len = Self::expect_length(buf)?;
        let data = buf.split_to(len);

        parse_frame(&mut data.as_ref()).map_err(|e| RespError::InvalidFrame(e.to_string()))
    }

    fn expect_length(buf: &[u8]) -> Result<usize, RespError> {
        parse_frame_length(buf)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;

    #[test]
    fn respv2_simple_string_len_should_work() {
        let buf = b"+OK\r\n";
        let len = RespFrame::expect_length(buf).unwrap();
        assert_eq!(len, 5);
    }

    #[test]
    fn respv2_simple_string_should_work() {
        let mut buf = BytesMut::from("+OK\r\n");
        let frame = RespFrame::decode(&mut buf).unwrap();
        assert_eq!(frame, RespFrame::SimpleString("OK".into()));
    }

    #[test]
    fn respv2_simple_error_should_work() {
        let mut buf = BytesMut::from("-ERR\r\n");
        let frame = RespFrame::decode(&mut buf).unwrap();
        assert_eq!(frame, RespFrame::Error("ERR".into()));
    }

    #[test]
    fn respv2_integer_should_work() {
        let mut buf = BytesMut::from(":123\r\n");
        let frame = RespFrame::decode(&mut buf).unwrap();
        assert_eq!(frame, RespFrame::Integer(123));
    }

    #[test]
    fn respv2_bulk_string_should_work() {
        let mut buf = BytesMut::from("$5\r\nhello\r\n");
        let frame = RespFrame::decode(&mut buf).unwrap();
        assert_eq!(frame, RespFrame::BulkString("hello".into()));
    }

    #[test]
    fn respv2_null_bulk_string_should_work() {
        let mut buf = BytesMut::from("$-1\r\n");
        let frame = RespFrame::decode(&mut buf).unwrap();
        assert_eq!(frame, RespFrame::NullBulkString(crate::RespNullBulkString));
    }

    #[test]
    fn respv2_array_should_work() {
        let mut buf = BytesMut::from("*2\r\n$3\r\nset\r\n$5\r\nhello\r\n");
        let frame = RespFrame::decode(&mut buf).unwrap();
        assert_eq!(
            frame,
            RespFrame::Array(
                vec![
                    RespFrame::BulkString("set".into()),
                    RespFrame::BulkString("hello".into())
                ]
                .into()
            )
        );
    }

    #[test]
    fn respv2_null_array_should_work() {
        let mut buf = BytesMut::from("*-1\r\n");
        let frame = RespFrame::decode(&mut buf).unwrap();
        assert_eq!(frame, RespFrame::NullArray(crate::RespNullArray));
    }

    #[test]
    fn respv2_null_should_work() {
        let mut buf = BytesMut::from("_\r\n");
        let frame = RespFrame::decode(&mut buf).unwrap();
        assert_eq!(frame, RespFrame::Null(crate::RespNull));
    }

    #[test]
    fn respv2_boolean_should_work() {
        let mut buf = BytesMut::from("#t\r\n");
        let frame = RespFrame::decode(&mut buf).unwrap();
        assert_eq!(frame, RespFrame::Boolean(true));
    }

    #[test]
    fn respv2_double_should_work() {
        let mut buf = BytesMut::from(",1.23\r\n");
        let frame = RespFrame::decode(&mut buf).unwrap();
        assert_eq!(frame, RespFrame::Double(1.23));
    }

    #[test]
    fn respv2_map_length_should_work() {
        let buf = b"%1\r\n+OK\r\n-ERR\r\n";
        let len = RespFrame::expect_length(buf).unwrap();
        assert_eq!(len, buf.len());
    }

    #[test]
    fn respv2_map_should_work() {
        let mut buf = BytesMut::from("%1\r\n+OK\r\n-ERR\r\n");
        let frame = RespFrame::decode(&mut buf).unwrap();
        let items: BTreeMap<String, RespFrame> =
            [("OK".to_string(), RespFrame::Error("ERR".into()))]
                .into_iter()
                .collect();
        assert_eq!(frame, RespFrame::Map(items.into()));
    }

    #[test]
    fn respv2_map_with_real_data_should_work() {
        let mut buf = BytesMut::from("%2\r\n+hello\r\n$5\r\nworld\r\n+foo\r\n$3\r\nbar\r\n");
        let frame = RespFrame::decode(&mut buf).unwrap();
        let items: BTreeMap<String, RespFrame> = [
            ("hello".to_string(), RespFrame::BulkString("world".into())),
            ("foo".to_string(), RespFrame::BulkString("bar".into())),
        ]
        .into_iter()
        .collect();
        assert_eq!(frame, RespFrame::Map(items.into()));
    }
}
