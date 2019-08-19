use failure::Error;

use networking::p2p::encoding::prelude::*;
use networking::p2p::message::BinaryMessage;

#[test]
fn can_deserialize_block_header() -> Result<(), Error> {
    let message_bytes = hex::decode("00006d6e0102dd00defaf70c53e180ea148b349a6feb4795610b2abc7b07fe91ce50a90814000000005c1276780432bc1d3a28df9a67b363aa1638f807214bb8987e5f9c0abcbd69531facffd1c80000001100000001000000000800000000000c15ef15a6f54021cb353780e2847fb9c546f1d72c1dc17c3db510f45553ce501ce1de000000000003c762c7df00a856b8bfcaf0676f069f825ca75f37f2bee9fe55ba109cec3d1d041d8c03519626c0c0faa557e778cb09d2e0c729e8556ed6a7a518c84982d1f2682bc6aa753f")?;
    let block_header = BlockHeader::from_bytes(message_bytes)?;
    assert_eq!(28014, block_header.get_level());
    assert_eq!(1, block_header.get_proto());
    assert_eq!(4, block_header.get_validation_pass());
    assert_eq!(2, block_header.get_fitness().len());
    assert_eq!("000000000003c762c7df00a856b8bfcaf0676f069f825ca75f37f2bee9fe55ba109cec3d1d041d8c03519626c0c0faa557e778cb09d2e0c729e8556ed6a7a518c84982d1f2682bc6aa753f", &hex::encode(&block_header.get_protocol_data()));

    Ok(())
}