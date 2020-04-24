// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT
#![feature(test)]
extern crate test;

use test::Bencher;

use serde::{Deserialize, Serialize};

use lazy_static::lazy_static;
use tezos_encoding::encoding::{Encoding, Field, HasEncoding};
use tezos_messages::p2p::binary_message::BinaryMessage;
use tezos_messages::p2p::binary_message::cache::{BinaryDataCache, CachedData, CacheReader, CacheWriter};

lazy_static! {
    pub static ref ENC: Encoding = Encoding::Obj(
        vec![
            Field::new("name", Encoding::String),
            Field::new("major", Encoding::Uint16),
            Field::new("minor", Encoding::Uint16),
        ]
    );
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct TestData {
    name: String,
    major: u16,
    minor: u16,
    #[serde(skip_serializing)]
    body: BinaryDataCache,
}

impl HasEncoding for TestData {
    fn encoding() -> Encoding {
        Encoding::Obj(
            vec![
                Field::new("name", Encoding::String),
                Field::new("major", Encoding::Uint16),
                Field::new("minor", Encoding::Uint16),
            ]
        )
    }
}

impl CachedData for TestData {
    #[inline]
    fn cache_reader(&self) -> &dyn CacheReader {
        &self.body
    }

    #[inline]
    fn cache_writer(&mut self) -> Option<&mut dyn CacheWriter> {
        Some(&mut self.body)
    }
}

#[bench]
fn bench_binary_writer(b: &mut Bencher) {
    b.iter(|| {
        let data = TestData { name: "aaa".to_string(), minor: 1, major: 5, body: Default::default() };
        assert!(data.as_bytes().is_ok());
    })
}

#[bench]
fn bench_binary_writer2(b: &mut Bencher) {
    b.iter(|| {
        let data = TestData { name: "aaa".to_string(), minor: 1, major: 5, body: Default::default() };
        assert!(tezos_encoding::binary_writer2::write(&data, &ENC).is_ok());
    })
}