use anyhow::Result;
use bytes::BytesMut;
use criterion::{criterion_group, criterion_main, Criterion};
use simple_redis::RespFrame;
// use std::hint::black_box;

const DATA: &str = "+OK\r\n-ERR\r\n:1000\r\n$5\r\nhello\r\n*3\r\n:1\r\n:2\r\n:3\r\n";

fn v1_decode() -> Result<Vec<RespFrame>> {
    use simple_redis::RespDecode;

    let mut buf = BytesMut::from(DATA);
    let mut frames = Vec::new();
    while !buf.is_empty() {
        let frame = RespFrame::decode(&mut buf)?;
        frames.push(frame);
    }

    Ok(frames)
}

fn v2_decode() -> Result<Vec<RespFrame>> {
    use simple_redis::RespDecodeV2;

    let mut buf = BytesMut::from(DATA);
    let mut frames = Vec::new();
    while !buf.is_empty() {
        let frame = RespFrame::decode(&mut buf)?;
        frames.push(frame);
    }
    Ok(frames)
}

fn criterion_benchmark(c: &mut Criterion) {
    c.bench_function("v1_decode", |b| b.iter(v1_decode));
    c.bench_function("v2_decode", |b| b.iter(v2_decode));
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
