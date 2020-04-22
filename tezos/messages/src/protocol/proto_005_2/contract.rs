// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::mem::size_of;
use std::sync::Arc;
use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::base::signature_public_key_hash::SignaturePublicKeyHash;
use crate::protocol::{ToRpcJsonMap, UniversalValue};

use tezos_encoding::{
    encoding::{Encoding, Field, HasEncoding, Tag, TagMap},
    types::BigInt,
};

use crate::p2p::binary_message::cache::{BinaryDataCache, CachedData, CacheReader, CacheWriter};

#[derive(Serialize, Debug, Clone)]
pub struct Script {
    code: Code,
    storage: Vec<MichelsonExpression>,

    #[serde(skip_serializing)]
    body: BinaryDataCache,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Code {
    code: Vec<MichelsonExpression>,

    #[serde(skip_serializing)]
    body: BinaryDataCache,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum MichelsonExpression {
    Int(MichelsonExpInt),
    String(MichelsonExpString),
    Array(Vec<Box<MichelsonExpression>>),
    Primitive(MichelsonExpPrimitive),
    PrimitiveWithAnotations(MichelsonExpPrimitiveWithAnotations),
    PrimitiveWithSingleArgument(Box<MichelsonExpPrimitiveWithSingleArgument>),
    PrimitiveWithTwoArguments(Box<MichelsonExpPrimitiveWithTwoArguments>),
    PrimitiveWithSingleArgumentAndAnotations(Box<MichelsonExpPrimitiveWithSingleArgumentAndAnotations>),
    PrimitiveWihtTwoArgumentsAndAnotations(Box<MichelsonExpPrimitiveWithTwoArgumentsAndAnotations>),
    PrimitiveWithNArguments(Box<MichelsonExpPrimitiveWithNArguments>),
}

impl HasEncoding for MichelsonExpression {
    fn encoding() -> Encoding {
        Encoding::Tags(
            size_of::<u8>(),
            TagMap::new(&[
                Tag::new(0x00, "Int", Encoding::Mutez),
                Tag::new(0x01, "String", Encoding::String),
                Tag::new(0x02, "Array", Encoding::dynamic(Encoding::list(Encoding::Lazy(Arc::new(MichelsonExpression::encoding))))),
                Tag::new(0x03, "Primitive", MichelsonExpPrimitive::encoding()),
                Tag::new(0x04, "PrimitiveWithAnotations", MichelsonExpPrimitiveWithAnotations::encoding()),
                Tag::new(0x05, "PrimitiveWithSingleArgument", MichelsonExpPrimitiveWithSingleArgument::encoding()),
                Tag::new(0x06, "PrimitiveWithSingleArgumentAndAnotations", MichelsonExpPrimitiveWithSingleArgumentAndAnotations::encoding()),
                Tag::new(0x07, "PrimitiveWithTwoArguments", MichelsonExpPrimitiveWithTwoArguments::encoding()),
                Tag::new(0x08, "PrimitiveWihtTwoArgumentsAndAnotations",  MichelsonExpPrimitiveWithTwoArgumentsAndAnotations::encoding()),
                Tag::new(0x09, "PrimitiveWithNArguments", MichelsonExpPrimitiveWithNArguments::encoding()),
                //Tag::new(0x10, "arbitrary_binary_data", Encoding::Bytes)
            ])
        )
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MichelsonExpInt {
    int: BigInt,
}

impl HasEncoding for MichelsonExpInt {
    fn encoding() -> Encoding {
        Encoding::Obj(vec![
            Field::new("int", Encoding::Mutez),
        ])
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MichelsonExpString {
    string: String,
}

impl HasEncoding for MichelsonExpString {
    fn encoding() -> Encoding {
        Encoding::Obj(vec![
            Field::new("string", Encoding::String),
        ])
    }
}

// #[derive(Serialize, Deserialize, Debug, Clone)]
// pub struct MichelsonExpArray {
//     array: Vec<MichelsonExpression>,
// }

// impl HasEncoding for MichelsonExpArray {
//     fn encoding() -> Encoding {
//         Encoding::dynamic(Encoding::list(Encoding::Lazy(Arc::new(MichelsonExpression::encoding))))
//         // Encoding::Lazy(Arc::new(Encoding::dynamic(Encoding::list(MichelsonExpression::encoding()))))
//     }
// }

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MichelsonExpPrimitive {
    prim: MichelsonPrimitive,
}

impl HasEncoding for MichelsonExpPrimitive {
    fn encoding() -> Encoding {
        Encoding::Obj(vec![
            Field::new("prim", Encoding::Tags(size_of::<u8>(), michelson_primitive_tag_map())),
        ])
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MichelsonExpPrimitiveWithAnotations {
    prim: MichelsonPrimitive,
    // TODO transform to array
    anots: String,
}

impl HasEncoding for MichelsonExpPrimitiveWithAnotations {
    fn encoding() -> Encoding {
        Encoding::Obj(vec![
            Field::new("prim", Encoding::Tags(size_of::<u8>(), michelson_primitive_tag_map())),
            Field::new("anots", Encoding::String),
        ])
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MichelsonExpPrimitiveWithSingleArgument {
    prim: MichelsonPrimitive,
    // TODO transform to array with this only element
    args: Box<MichelsonExpression>,
}

impl HasEncoding for MichelsonExpPrimitiveWithSingleArgument {
    fn encoding() -> Encoding {
        Encoding::Obj(vec![
            Field::new("prim", Encoding::Tags(size_of::<u8>(), michelson_primitive_tag_map())),
            Field::new("args", Encoding::Lazy(Arc::new(MichelsonExpression::encoding))),
        ])
    }
}

// MichelsonExpPrimitiveWithTwoArguments
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MichelsonExpPrimitiveWithTwoArguments {
    prim: MichelsonPrimitive,
    args: [Box<MichelsonExpression>; 2],
}

impl HasEncoding for MichelsonExpPrimitiveWithTwoArguments {
    fn encoding() -> Encoding {
        Encoding::Obj(vec![
            Field::new("prim", Encoding::Tags(size_of::<u8>(), michelson_primitive_tag_map())),
            Field::new("args", Encoding::list(Encoding::Lazy(Arc::new(MichelsonExpression::encoding)))),
        ])
    }
}


#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MichelsonExpPrimitiveWithSingleArgumentAndAnotations {
    prim: MichelsonPrimitive,
    // TODO transform to array with this only element
    args: Box<MichelsonExpression>,
    // TODO transform to array
    anots: String,
}

impl HasEncoding for MichelsonExpPrimitiveWithSingleArgumentAndAnotations {
    fn encoding() -> Encoding {
        Encoding::Obj(vec![
            Field::new("prim", Encoding::Tags(size_of::<u8>(), michelson_primitive_tag_map())),
            Field::new("args", Encoding::Lazy(Arc::new(MichelsonExpression::encoding))),
            Field::new("anots", Encoding::String),
        ])
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MichelsonExpPrimitiveWithTwoArgumentsAndAnotations {
    prim: MichelsonPrimitive,
    args: [Box<MichelsonExpression>; 2],
    // TODO transform to array
    anots: String,
}

impl HasEncoding for MichelsonExpPrimitiveWithTwoArgumentsAndAnotations {
    fn encoding() -> Encoding {
        Encoding::Obj(vec![
            Field::new("prim", Encoding::Tags(size_of::<u8>(), michelson_primitive_tag_map())),
            Field::new("args", Encoding::list(Encoding::Lazy(Arc::new(MichelsonExpression::encoding)))),
            Field::new("anots", Encoding::String),
        ])
    }
}

// MichelsonExpPrimitiveWithNArguments

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MichelsonExpPrimitiveWithNArguments {
    prim: MichelsonPrimitive,
    args: Vec<Box<MichelsonExpression>>,
}

impl HasEncoding for MichelsonExpPrimitiveWithNArguments {
    fn encoding() -> Encoding {
        Encoding::Obj(vec![
            Field::new("prim", Encoding::Tags(size_of::<u8>(), michelson_primitive_tag_map())),
            Field::new("args", Encoding::dynamic(Encoding::list(Encoding::Lazy(Arc::new(MichelsonExpression::encoding))))),
        ])
    }
}

// #[derive(Serialize, Deserialize, Debug, Clone)]
// pub struct MichelsonExpressionPlaceHolder {
//     int: Option<BigInt>,
//     string: Option<String>,
//     prim: Option<MichelsonPrimitive>,
//     args: Option<Vec<Box<MichelsonExpressionPlaceHolder>>>,
//     anots: Option<Vec<String>>,

//     // #[serde(skip_serializing)]
//     // body: BinaryDataCache,
// }

impl CachedData for Code {
    #[inline]
    fn cache_reader(&self) -> &dyn CacheReader {
        &self.body
    }

    #[inline]
    fn cache_writer(&mut self) -> Option<&mut dyn CacheWriter> {
        Some(&mut self.body)
    }
}

impl HasEncoding for Code {
    fn encoding() -> Encoding {
        Encoding::Obj(vec![
            Field::new("code", Encoding::dynamic(Encoding::list(MichelsonExpression::encoding())))
        ])
    }
}

// #[derive(Serialize, Debug, Clone)]
// pub struct Storage {
//     storage: String,

//     #[serde(skip_serializing)]
//     body: BinaryDataCache,
// }

// impl HasEncoding for Storage {
//     fn encoding() -> Encoding {
//         // TODO ?
//         Encoding::dynamic(Encoding::list(MichelsonExpressionPlaceHolder::encoding()))
//     }
// }

// impl CachedData for Storage {
//     #[inline]
//     fn cache_reader(&self) -> &dyn CacheReader {
//         &self.body
//     }

//     #[inline]
//     fn cache_writer(&mut self) -> Option<&mut dyn CacheWriter> {
//         Some(&mut self.body)
//     }
// }

// pub fn array_encoding() -> Encoding {
    
// }

// pub fn single_prim_with_anot_encoding() -> Encoding {
//     Encoding::Obj(vec![
//         Field::new("prim", Encoding::Enum),
//         // TODO this needs post processing, multiple anots are encoded as one stirng seperated with <space> character 
//         Field::new("anot", Encoding::String),
//     ])
// }

// pub fn single_prim_with_single_args_encoding() -> Encoding {
//     Encoding::Obj(vec![
//         Field::new("prim", Encoding::Enum),
//         Field::new("args", Encoding::Lazy(Arc::new(MichelsonExpressionPlaceHolder::encoding))),
//     ])
// }

// pub fn single_prim_with_two_args_encoding() -> Encoding {
//     Encoding::Obj(vec![
//         Field::new("prim", Encoding::Enum),
//         // Field::new("args", Encoding::dynamic(Encoding::list(Encoding::Lazy(Arc::new(MichelsonExpressionPlaceHolder::encoding))))),
//         Field::new("args", Encoding::list(Encoding::Lazy(Arc::new(MichelsonExpressionPlaceHolder::encoding)))),
//     ])
// }

// pub fn single_prim_with_two_args_with_anot() -> Encoding {
//     Encoding::Obj(vec![
//         Field::new("prim", Encoding::Enum),
//         Field::new("args", Encoding::list(Encoding::Lazy(Arc::new(MichelsonExpressionPlaceHolder::encoding)))),
//         Field::new("anot", Encoding::String),
//     ])
// }

// pub fn single_prim_with_one_arg_with_anot() -> Encoding {
//     Encoding::Obj(vec![
//         Field::new("prim", Encoding::Enum),
//         Field::new("args", Encoding::Lazy(Arc::new(MichelsonExpressionPlaceHolder::encoding))),
//         Field::new("anot", Encoding::String),
//     ])
// }

// pub fn single_prim_with_n_args_with_optional_anot() -> Encoding {
//     Encoding::Obj(vec![
//         Field::new("prim", Encoding::Enum),
//         Field::new("args", Encoding::Lazy(Arc::new(MichelsonExpressionPlaceHolder::encoding))),

//         // TODO: make this optional
//         Field::new("anot", Encoding::dynamic(Encoding::list(Encoding::String))),
//     ])
// }

// pub fn michelson_expression_encoding() -> Encoding {
//     Encoding::Tags(
//         size_of::<u8>(),
//         TagMap::new(&[
//             Tag::new(0x00, "int", Encoding::Mutez),
//             Tag::new(0x01, "string", Encoding::String),
//             Tag::new(0x02, "array", Encoding::Lazy(Arc::new(array_encoding))),
//             Tag::new(0x03, "prim", Encoding::Enum),
//             Tag::new(0x04, "single_prim_with_anot", single_prim_with_anot_encoding()),
//             Tag::new(0x05, "single_prim_with_single_arg", single_prim_with_single_args_encoding()),
//             Tag::new(0x06, "single_prim_with_single_arg_with_anot", single_prim_with_one_arg_with_anot()),
//             Tag::new(0x07, "single_prim_with_two_args", single_prim_with_two_args_encoding()),
//             Tag::new(0x08, "single_prim_with_two_args_with_anot",  single_prim_with_two_args_with_anot()),
//             Tag::new(0x09, "single_prim_with_n_args", single_prim_with_n_args_with_optional_anot()),
//             //Tag::new(0x10, "arbitrary_binary_data", Encoding::Bytes)
//         ])
//     )
// }

pub fn michelson_primitive_tag_map() -> TagMap {
    let primitive_vec = vec![
        MichelsonPrimitive::parameter,
        MichelsonPrimitive::storage,
        MichelsonPrimitive::code,
        MichelsonPrimitive::False,
        MichelsonPrimitive::Elt,
        MichelsonPrimitive::Left,
        MichelsonPrimitive::None,
        MichelsonPrimitive::Pair,
        MichelsonPrimitive::Right,
        MichelsonPrimitive::Some,
        MichelsonPrimitive::True,
        MichelsonPrimitive::Unit,
        MichelsonPrimitive::PACK,
        MichelsonPrimitive::UNPACK,
        MichelsonPrimitive::BLAKE2B,
        MichelsonPrimitive::SHA256,
        MichelsonPrimitive::SHA512,
        MichelsonPrimitive::ABS,
        MichelsonPrimitive::ADD,
        MichelsonPrimitive::AMOUNT,
        MichelsonPrimitive::AND,
        MichelsonPrimitive::BALANCE,
        MichelsonPrimitive::CAR,
        MichelsonPrimitive::CDR,
        MichelsonPrimitive::CHECK_SIGNATURE,
        MichelsonPrimitive::COMPARE,
        MichelsonPrimitive::CONCAT,
        MichelsonPrimitive::CONS,
        MichelsonPrimitive::CREATE_ACCOUNT,
        MichelsonPrimitive::CREATE_CONTRACT,
        MichelsonPrimitive::IMPLICIT_ACCOUNT,
        MichelsonPrimitive::DIP,
        MichelsonPrimitive::DROP,
        MichelsonPrimitive::DUP,
        MichelsonPrimitive::EDIV,
        MichelsonPrimitive::EMPTY_MAP,
        MichelsonPrimitive::EMPTY_SET,
        MichelsonPrimitive::EQ,
        MichelsonPrimitive::EXEC,
        MichelsonPrimitive::FAILWITH,
        MichelsonPrimitive::GE,
        MichelsonPrimitive::GET,
        MichelsonPrimitive::GT,
        MichelsonPrimitive::HASH_KEY,
        MichelsonPrimitive::IF,
        MichelsonPrimitive::IF_CONS,
        MichelsonPrimitive::IF_LEFT,
        MichelsonPrimitive::IF_NONE,
        MichelsonPrimitive::INT,
        MichelsonPrimitive::LAMBDA,
        MichelsonPrimitive::LE,
        MichelsonPrimitive::LEFT,
        MichelsonPrimitive::LOOP,
        MichelsonPrimitive::LSL,
        MichelsonPrimitive::LSR,
        MichelsonPrimitive::LT,
        MichelsonPrimitive::MAP,
        MichelsonPrimitive::MEM,
        MichelsonPrimitive::MUL,
        MichelsonPrimitive::NEG,
        MichelsonPrimitive::NEQ,
        MichelsonPrimitive::NIL,
        MichelsonPrimitive::NONE,
        MichelsonPrimitive::NOT,
        MichelsonPrimitive::NOW,
        MichelsonPrimitive::OR,
        MichelsonPrimitive::PAIR,
        MichelsonPrimitive::PUSH,
        MichelsonPrimitive::RIGHT,
        MichelsonPrimitive::SIZE,
        MichelsonPrimitive::SOME,
        MichelsonPrimitive::SOURCE,
        MichelsonPrimitive::SENDER,
        MichelsonPrimitive::SELF,
        MichelsonPrimitive::STEPS_TO_QUOTA,
        MichelsonPrimitive::SUB,
        MichelsonPrimitive::SWAP,
        MichelsonPrimitive::TRANSFER_TOKENS,
        MichelsonPrimitive::SET_DELEGATE,
        MichelsonPrimitive::UNIT,
        MichelsonPrimitive::UPDATE,
        MichelsonPrimitive::XOR,
        MichelsonPrimitive::ITER,
        MichelsonPrimitive::LOOP_LEFT,
        MichelsonPrimitive::ADDRESS,
        MichelsonPrimitive::CONTRACT,
        MichelsonPrimitive::ISNAT,
        MichelsonPrimitive::CAST,
        MichelsonPrimitive::RENAME,
        MichelsonPrimitive::bool,
        MichelsonPrimitive::contract,
        MichelsonPrimitive::int,
        MichelsonPrimitive::key,
        MichelsonPrimitive::key_hash,
        MichelsonPrimitive::lambda,
        MichelsonPrimitive::list,
        MichelsonPrimitive::map,
        MichelsonPrimitive::big_map,
        MichelsonPrimitive::nat,
        MichelsonPrimitive::option,
        MichelsonPrimitive::or,
        MichelsonPrimitive::pair,
        MichelsonPrimitive::set,
        MichelsonPrimitive::signature,
        MichelsonPrimitive::string,
        MichelsonPrimitive::bytes,
        MichelsonPrimitive::mutez,
        MichelsonPrimitive::timestamp,
        MichelsonPrimitive::unit,
        MichelsonPrimitive::operation,
        MichelsonPrimitive::address,
        MichelsonPrimitive::SLICE,
        MichelsonPrimitive::DIG,
        MichelsonPrimitive::DUG,
        MichelsonPrimitive::EMPTY_BIG_MAP,
        MichelsonPrimitive::APPLY,
        MichelsonPrimitive::chain_id,
        MichelsonPrimitive::CHAIN_ID,
        ];
        
        // let mut primitive_tags: TagMap = Default::default();
        // let mut tag_hash_map: HashMap<u16, &'static str> = Default::default();
        let mut tag_vec: Vec<Tag> = Default::default();

        let mut counter: u16 = 0;
        for element in primitive_vec {
            tag_vec.push(Tag::new(counter, element.as_custom_named_variant(), Encoding::Unit));
            counter += 1;
        }
        TagMap::new(&tag_vec)
}

#[allow(non_camel_case_types)]
#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
pub enum MichelsonPrimitive{
    parameter,
    storage,
    code,
    False,
    Elt,
    Left,
    None,
    Pair,
    Right,
    Some,
    True,
    Unit,
    PACK,
    UNPACK,
    BLAKE2B,
    SHA256,
    SHA512,
    ABS,
    ADD,
    AMOUNT,
    AND,
    BALANCE,
    CAR,
    CDR,
    CHECK_SIGNATURE,
    COMPARE,
    CONCAT,
    CONS,
    CREATE_ACCOUNT,
    CREATE_CONTRACT,
    IMPLICIT_ACCOUNT,
    DIP,
    DROP,
    DUP,
    EDIV,
    EMPTY_MAP,
    EMPTY_SET,
    EQ,
    EXEC,
    FAILWITH,
    GE,
    GET,
    GT,
    HASH_KEY,
    IF,
    IF_CONS,
    IF_LEFT,
    IF_NONE,
    INT,
    LAMBDA,
    LE,
    LEFT,
    LOOP,
    LSL,
    LSR,
    LT,
    MAP,
    MEM,
    MUL,
    NEG,
    NEQ,
    NIL,
    NONE,
    NOT,
    NOW,
    OR,
    PAIR,
    PUSH,
    RIGHT,
    SIZE,
    SOME,
    SOURCE,
    SENDER,
    SELF,
    STEPS_TO_QUOTA,
    SUB,
    SWAP,
    TRANSFER_TOKENS,
    SET_DELEGATE,
    UNIT,
    UPDATE,
    XOR,
    ITER,
    LOOP_LEFT,
    ADDRESS,
    CONTRACT,
    ISNAT,
    CAST,
    RENAME,
    bool,
    contract,
    int,
    key,
    key_hash,
    lambda,
    list,
    map,
    big_map,
    nat,
    option,
    or,
    pair,
    set,
    signature,
    string,
    bytes,
    mutez,
    timestamp,
    unit,
    operation,
    address,
    SLICE,
    DIG,
    DUG,
    EMPTY_BIG_MAP,
    APPLY,
    chain_id,
    CHAIN_ID,
}

impl MichelsonPrimitive {
    pub fn as_custom_named_variant(&self) -> &'static str {
        match self {
            MichelsonPrimitive::parameter => "parameter",
            MichelsonPrimitive::storage => "storage",
            MichelsonPrimitive::code => "code",
            MichelsonPrimitive::False => "False",
            MichelsonPrimitive::Elt => "Elt",
            MichelsonPrimitive::Left => "Left",
            MichelsonPrimitive::None => "None",
            MichelsonPrimitive::Pair => "Pair",
            MichelsonPrimitive::Right => "Right",
            MichelsonPrimitive::Some => "Some",
            MichelsonPrimitive::True => "True",
            MichelsonPrimitive::Unit => "Unit",
            MichelsonPrimitive::PACK => "PACK",
            MichelsonPrimitive::UNPACK => "UNAPCK",
            MichelsonPrimitive::BLAKE2B => "BLAKE2B",
            MichelsonPrimitive::SHA256 => "SHA256",
            MichelsonPrimitive::SHA512 => "SHA512",
            MichelsonPrimitive::ABS => "ABS",
            MichelsonPrimitive::ADD => "ADD",
            MichelsonPrimitive::AMOUNT => "AMOUNT",
            MichelsonPrimitive::AND => "AND",
            MichelsonPrimitive::BALANCE => "BALANCE",
            MichelsonPrimitive::CAR => "CAR",
            MichelsonPrimitive::CDR => "CDR",
            MichelsonPrimitive::CHAIN_ID => "CHAIN_ID",
            MichelsonPrimitive::CHECK_SIGNATURE => "CHECK_SIGNATURE",
            MichelsonPrimitive::COMPARE => "COMPARE",
            MichelsonPrimitive::CONCAT => "CONCAT",
            MichelsonPrimitive::CONS => "CONS",
            MichelsonPrimitive::CREATE_ACCOUNT => "CREATE_ACCOUNT",
            MichelsonPrimitive::CREATE_CONTRACT => "CREATE_CONTRACT",
            MichelsonPrimitive::IMPLICIT_ACCOUNT => "IMPLICIT_ACCOUNT",
            MichelsonPrimitive::DIP => "DIP",
            MichelsonPrimitive::DROP => "DROP",
            MichelsonPrimitive::DUP => "DUP",
            MichelsonPrimitive::EDIV => "EDIV",
            MichelsonPrimitive::EMPTY_BIG_MAP => "EMPTY_BIG_MAP",
            MichelsonPrimitive::EMPTY_MAP => "EMPTY_MAP",
            MichelsonPrimitive::EMPTY_SET => "EMPTY_SET",
            MichelsonPrimitive::EQ => "EQ",
            MichelsonPrimitive::EXEC => "EXEC",
            MichelsonPrimitive::APPLY => "APPLY",
            MichelsonPrimitive::FAILWITH => "FAILWITH",
            MichelsonPrimitive::GE => "GE",
            MichelsonPrimitive::GET => "GET",
            MichelsonPrimitive::GT => "GT",
            MichelsonPrimitive::HASH_KEY => "HASH_KEY",
            MichelsonPrimitive::IF => "IF",
            MichelsonPrimitive::IF_CONS => "IF_CONS",
            MichelsonPrimitive::IF_LEFT => "IF_LEFT",
            MichelsonPrimitive::IF_NONE => "IF_NONE",
            MichelsonPrimitive::INT => "INT",
            MichelsonPrimitive::LAMBDA => "LAMBDA",
            MichelsonPrimitive::LE => "LE",
            MichelsonPrimitive::LEFT => "LEFT",
            MichelsonPrimitive::LOOP => "LOOP",
            MichelsonPrimitive::LSL => "LSL",
            MichelsonPrimitive::LSR => "LSR",
            MichelsonPrimitive::LT => "LT",
            MichelsonPrimitive::MAP => "MAP",
            MichelsonPrimitive::MEM => "MEM",
            MichelsonPrimitive::MUL => "MUL",
            MichelsonPrimitive::NEG => "NEG",
            MichelsonPrimitive::NEQ => "NEQ",
            MichelsonPrimitive::NIL => "NIL",
            MichelsonPrimitive::NONE => "NONE",
            MichelsonPrimitive::NOT => "NOT",
            MichelsonPrimitive::NOW => "NOW",
            MichelsonPrimitive::OR => "OR",
            MichelsonPrimitive::PAIR => "PAIR",
            MichelsonPrimitive::PUSH => "PUSH",
            MichelsonPrimitive::RIGHT => "RIGHT",
            MichelsonPrimitive::SIZE => "SIZE",
            MichelsonPrimitive::SOME => "SOME",
            MichelsonPrimitive::SOURCE => "SOURCE",
            MichelsonPrimitive::SENDER => "SENDER",
            MichelsonPrimitive::SELF => "SELF",
            MichelsonPrimitive::SLICE => "SLICE",
            MichelsonPrimitive::STEPS_TO_QUOTA => "STEPS_TO_QUOTA",
            MichelsonPrimitive::SUB => "SUB",
            MichelsonPrimitive::SWAP => "SWAP",
            MichelsonPrimitive::TRANSFER_TOKENS => "TRANSFER_TOKENS",
            MichelsonPrimitive::SET_DELEGATE => "SET_DELEGATE",
            MichelsonPrimitive::UNIT => "UNIT",
            MichelsonPrimitive::UPDATE => "UPDATE",
            MichelsonPrimitive::XOR => "XOR",
            MichelsonPrimitive::ITER => "ITER",
            MichelsonPrimitive::LOOP_LEFT => "LOOP_LEFT",
            MichelsonPrimitive::ADDRESS => "ADDRESS",
            MichelsonPrimitive::CONTRACT => "CONTRACT",
            MichelsonPrimitive::ISNAT => "ISNAT",
            MichelsonPrimitive::CAST => "CAST",
            MichelsonPrimitive::RENAME => "RENAME",
            MichelsonPrimitive::DIG => "DIG",
            MichelsonPrimitive::DUG => "DUG",
            MichelsonPrimitive::bool => "bool",
            MichelsonPrimitive::contract => "contract",
            MichelsonPrimitive::int => "int",
            MichelsonPrimitive::key => "key",
            MichelsonPrimitive::key_hash => "key_hash",
            MichelsonPrimitive::lambda => "lambda",
            MichelsonPrimitive::list => "list",
            MichelsonPrimitive::map => "map",
            MichelsonPrimitive::big_map => "big_map",
            MichelsonPrimitive::nat => "nat",
            MichelsonPrimitive::option => "option",
            MichelsonPrimitive::or => "or",
            MichelsonPrimitive::pair => "pair",
            MichelsonPrimitive::set => "set",
            MichelsonPrimitive::signature => "signature",
            MichelsonPrimitive::string => "string",
            MichelsonPrimitive::bytes => "bytes",
            MichelsonPrimitive::mutez => "mutez",
            MichelsonPrimitive::timestamp => "timestamp",
            MichelsonPrimitive::unit => "unit",
            MichelsonPrimitive::operation => "operation",
            MichelsonPrimitive::address => "address",
            MichelsonPrimitive::chain_id => "chain_id",
        }
    }

    // pub fn get_tag(&self) -> u16 {
        
    //     // TODO: make this safe
    //     primitive_vec.iter().position(|r| r == self).unwrap() as u16
    // }
}

// impl CachedData for MichelsonExpressionPlaceHolder {
//     #[inline]
//     fn cache_reader(&self) -> &dyn CacheReader {
//         &self.body
//     }

//     #[inline]
//     fn cache_writer(&mut self) -> Option<&mut dyn CacheWriter> {
//         Some(&mut self.body)
//     }
// }

#[derive(Serialize, Debug, Clone)]
pub struct Contract {
    pub balance: BigInt,
    pub delegate: Option<SignaturePublicKeyHash>,
    pub script: Vec<Script>,
    pub counter: Option<BigInt>,
}