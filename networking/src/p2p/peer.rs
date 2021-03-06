// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::convert::TryFrom;
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Duration;

use failure::{Error, Fail};
use futures::lock::Mutex;
use riker::actors::*;
use slog::{debug, info, Logger, trace, warn};
use tokio::net::TcpStream;
use tokio::runtime::Handle;
use tokio::time::timeout;

use crypto::crypto_box::precompute;
use crypto::hash::HashType;
use crypto::nonce::{self, Nonce, NoncePair};
use storage::p2p_message_storage::P2PMessageStorage;
use tezos_encoding::binary_reader::BinaryReaderError;
use tezos_messages::p2p::binary_message::{BinaryChunk, BinaryChunkError, BinaryMessage};
use tezos_messages::p2p::encoding::ack::NackInfo;
use tezos_messages::p2p::encoding::prelude::*;

use super::network_channel::{NetworkChannelRef, NetworkChannelTopic, PeerBootstrapped, PeerMessageReceived};
use super::stream::{EncryptedMessageReader, EncryptedMessageWriter, MessageStream, StreamError};

const SUPPORTED_DISTRIBUTED_DB_VERSION: u16 = 0;
const SUPPORTED_P2P_VERSION: u16 = 1;

const IO_TIMEOUT: Duration = Duration::from_secs(6);
const READ_TIMEOUT_LONG: Duration = Duration::from_secs(30);

static ACTOR_ID_GENERATOR: AtomicU64 = AtomicU64::new(0);

pub type PeerId = String;
pub type PublicKey = Vec<u8>;

#[derive(Debug, Fail)]
enum PeerError {
    #[fail(display = "Unsupported protocol - supported_version: {} vs. {}", supported_version, incompatible_versions)]
    UnsupportedProtocol {
        supported_version: String,
        incompatible_versions: String,
    },
    #[fail(display = "Received NACK from remote peer")]
    NackReceived,
    #[fail(display = "Received NACK from remote peer with info: {:?}", nack_info)]
    NackWithMotiveReceived {
        nack_info: NackInfo
    },
    #[fail(display = "Failed to create precomputed key")]
    FailedToPrecomputeKey,
    #[fail(display = "Network error: {}", message)]
    NetworkError {
        error: Error,
        message: &'static str,
    },
    #[fail(display = "Message serialization error")]
    SerializationError {
        error: tezos_encoding::ser::Error
    },
    #[fail(display = "Message deserialization error")]
    DeserializationError {
        error: BinaryReaderError
    },
}

impl From<tezos_encoding::ser::Error> for PeerError {
    fn from(error: tezos_encoding::ser::Error) -> Self {
        PeerError::SerializationError { error }
    }
}

impl From<BinaryReaderError> for PeerError {
    fn from(error: BinaryReaderError) -> Self {
        PeerError::DeserializationError { error }
    }
}

impl From<std::io::Error> for PeerError {
    fn from(error: std::io::Error) -> Self {
        PeerError::NetworkError { error: error.into(), message: "Network error" }
    }
}

impl From<StreamError> for PeerError {
    fn from(error: StreamError) -> Self {
        PeerError::NetworkError { error: error.into(), message: "Stream error" }
    }
}

impl From<BinaryChunkError> for PeerError {
    fn from(error: BinaryChunkError) -> Self {
        PeerError::NetworkError { error: error.into(), message: "Binary chunk error" }
    }
}

impl slog::Value for PeerError {
    fn serialize(&self, _record: &slog::Record, key: slog::Key, serializer: &mut dyn slog::Serializer) -> slog::Result {
        serializer.emit_arguments(key, &format_args!("{}", self))
    }
}

impl From<tokio::time::Elapsed> for PeerError {
    fn from(timeout: tokio::time::Elapsed) -> Self {
        PeerError::NetworkError {
            message: "Connection timeout",
            error: timeout.into(),
        }
    }
}

/// Commands peer actor to initialize bootstrapping process with a remote peer.
#[derive(Clone, Debug)]
pub struct Bootstrap {
    stream: Arc<Mutex<Option<TcpStream>>>,
    address: SocketAddr,
    incoming: bool,
}

impl Bootstrap {
    pub fn incoming(stream: Arc<Mutex<Option<TcpStream>>>, address: SocketAddr) -> Self {
        Bootstrap { stream, address, incoming: true }
    }

    pub fn outgoing(stream: TcpStream, address: SocketAddr) -> Self {
        Bootstrap { stream: Arc::new(Mutex::new(Some(stream))), address, incoming: false }
    }
}

/// Commands peer actor to send a p2p message to a remote peer.
#[derive(Clone, Debug)]
pub struct SendMessage {
    /// Message is wrapped in `Arc` to avoid excessive cloning.
    message: Arc<PeerMessageResponse>
}

impl SendMessage {
    pub fn new(msg: PeerMessageResponse) -> Self {
        SendMessage { message: Arc::new(msg) }
    }
}

#[derive(Clone)]
struct Network {
    /// Message receiver boolean indicating whether
    /// more messages should be received from network
    rx_run: Arc<AtomicBool>,
    /// Message sender
    tx: Arc<Mutex<Option<EncryptedMessageWriter>>>,
    /// Socket address of the peer
    socket_address: SocketAddr,
}

/// Local node info
pub struct Local {
    /// port where remote node can establish new connection
    listener_port: u16,
    /// our public key
    public_key: String,
    /// our secret key
    secret_key: String,
    /// proof of work
    proof_of_work_stamp: String,
    /// version of network protocol
    version: String,
}

pub type PeerRef = ActorRef<PeerMsg>;

/// Represents a single p2p peer.
#[actor(Bootstrap, SendMessage)]
pub struct Peer {
    /// All events generated by the peer will end up in this channel
    network_channel: NetworkChannelRef,
    /// Local node info
    local: Arc<Local>,
    /// Network IO
    net: Network,
    /// Tokio task executor
    tokio_executor: Handle,
    /// Msg storage
    msg_store: P2PMessageStorage,
    remote_addr: SocketAddr,
}

impl Peer {
    /// Create instance of a peer actor.
    pub fn actor(sys: &impl ActorRefFactory,
                 network_channel: NetworkChannelRef,
                 listener_port: u16,
                 public_key: &str,
                 secret_key: &str,
                 proof_of_work_stamp: &str,
                 version: &str,
                 tokio_executor: Handle,
                 socket_address: &SocketAddr,
                 p2p_msg_store: P2PMessageStorage) -> Result<PeerRef, CreateError>
    {
        let info = Local {
            listener_port,
            proof_of_work_stamp: proof_of_work_stamp.into(),
            public_key: public_key.into(),
            secret_key: secret_key.into(),
            version: version.into(),
        };
        let props = Props::new_args(Peer::new, (network_channel, Arc::new(info), tokio_executor, *socket_address, p2p_msg_store));
        let actor_id = ACTOR_ID_GENERATOR.fetch_add(1, Ordering::SeqCst);
        sys.actor_of(props, &format!("peer-{}", actor_id))
    }

    fn new((event_channel, info, tokio_executor, socket_address, msg_store): (NetworkChannelRef, Arc<Local>, Handle, SocketAddr, P2PMessageStorage)) -> Self {
        Peer {
            network_channel: event_channel,
            local: info,
            net: Network {
                rx_run: Arc::new(AtomicBool::new(false)),
                tx: Arc::new(Mutex::new(None)),
                socket_address,
            },
            tokio_executor,
            msg_store,
            remote_addr: SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(0, 0, 0, 0), 0)),
        }
    }
}

impl Actor for Peer {
    type Msg = PeerMsg;

    fn post_stop(&mut self) {
        self.net.rx_run.store(false, Ordering::Release);
    }

    fn recv(&mut self, ctx: &Context<Self::Msg>, msg: Self::Msg, sender: Sender) {
        // Use the respective Receive<T> implementation
        self.receive(ctx, msg, sender);
    }
}

impl Receive<Bootstrap> for Peer {
    type Msg = PeerMsg;

    fn receive(&mut self, ctx: &Context<Self::Msg>, msg: Bootstrap, _sender: Sender) {
        let info = self.local.clone();
        let myself = ctx.myself();
        let system = ctx.system.clone();
        let net = self.net.clone();
        let network_channel = self.network_channel.clone();
        self.remote_addr = msg.address;

        let store = self.msg_store.clone();
        self.tokio_executor.spawn(async move {
            async fn setup_net(net: &Network, tx: EncryptedMessageWriter) {
                net.rx_run.store(true, Ordering::Release);
                *net.tx.lock().await = Some(tx);
            }

            let peer_address = msg.address;
            debug!(system.log(), "Bootstrapping"; "ip" => &peer_address, "peer" => myself.name());
            match bootstrap(msg, info, system.log(), store.clone()).await {
                Ok(BootstrapOutput(rx, tx, public_key)) => {
                    debug!(system.log(), "Bootstrap successful"; "ip" => &peer_address, "peer" => myself.name());
                    setup_net(&net, tx).await;

                    let peer_id = HashType::CryptoboxPublicKeyHash.bytes_to_string(&public_key);
                    // notify that peer was bootstrapped successfully
                    network_channel.tell(Publish {
                        msg: PeerBootstrapped::Success {
                            peer: myself.clone(),
                            peer_id: peer_id.clone(),
                        }.into(),
                        topic: NetworkChannelTopic::NetworkEvents.into(),
                    }, Some(myself.clone().into()));

                    // begin to process incoming messages in a loop
                    let log = system.log().new(slog::o!("peer" => peer_id));
                    begin_process_incoming(rx, net.rx_run, myself.clone(), network_channel, log, store.clone(), peer_address.clone()).await;
                    // connection to peer was closed, stop this actor
                    system.stop(myself);
                }
                Err(err) => {
                    info!(system.log(), "Connection to peer failed"; "reason" => &err, "ip" => &peer_address, "peer" => myself.name());

                    let potential_peers = match err {
                        PeerError::NackWithMotiveReceived { nack_info } => Some(nack_info.potential_peers_to_connect),
                        _ => None
                    };

                    // notify that peer failed at bootstrap process
                    network_channel.tell(Publish {
                        msg: PeerBootstrapped::Failure {
                            address: peer_address,
                            potential_peers_to_connect: potential_peers
                        }.into(),
                        topic: NetworkChannelTopic::NetworkEvents.into(),
                    }, Some(myself.clone().into()));

                    system.stop(myself);
                }
            }
        });
    }
}

impl Receive<SendMessage> for Peer {
    type Msg = PeerMsg;

    fn receive(&mut self, ctx: &Context<Self::Msg>, msg: SendMessage, _sender: Sender) {
        let system = ctx.system.clone();
        let myself = ctx.myself();
        let tx = self.net.tx.clone();
        let mut store = self.msg_store.clone();
        let addr = self.remote_addr;
        self.tokio_executor.spawn(async move {
            let mut tx_lock = tx.lock().await;
            if let Some(tx) = tx_lock.as_mut() {
                let _ = store.store_peer_message(msg.message.messages(), true, addr);
                match timeout(IO_TIMEOUT, tx.write_message(&*msg.message)).await {
                    Ok(write_result) => {
                        if let Err(e) = write_result {
                            warn!(system.log(), "Failed to send message"; "reason" => e);
                            system.stop(myself);
                        }
                    }
                    Err(_) => {
                        warn!(system.log(), "Failed to send message"; "reason" => "timeout");
                        system.stop(myself);
                    }
                }
            }
        });
    }
}

/// Output values of the successful bootstrap process
struct BootstrapOutput(EncryptedMessageReader, EncryptedMessageWriter, PublicKey);

async fn bootstrap(msg: Bootstrap, info: Arc<Local>, log: Logger, mut storage: P2PMessageStorage) -> Result<BootstrapOutput, PeerError> {
    let addr = msg.address;
    let (mut msg_rx, mut msg_tx) = {
        let stream = msg.stream.lock().await.take().expect("Someone took ownership of the socket before the Peer");
        let msg_reader: MessageStream = stream.into();
        msg_reader.split()
    };

    let supported_protocol_version = Version::new(info.version.clone(), SUPPORTED_DISTRIBUTED_DB_VERSION, SUPPORTED_P2P_VERSION);

    // send connection message
    let connection_message = ConnectionMessage::new(
        info.listener_port,
        &info.public_key,
        &info.proof_of_work_stamp,
        &Nonce::random().get_bytes(),
        vec![supported_protocol_version.clone()]);
    let _ = storage.store_connection_message(&connection_message, false, addr);
    let connection_message_sent = {
        let connection_message_bytes = BinaryChunk::from_content(&connection_message.as_bytes()?)?;
        match timeout(IO_TIMEOUT, msg_tx.write_message(&connection_message_bytes)).await? {
            Ok(_) => connection_message_bytes,
            Err(e) => return Err(PeerError::NetworkError { error: e.into(), message: "Failed to transfer connection message" })
        }
    };

    // receive connection message
    let received_connection_msg = match timeout(IO_TIMEOUT, msg_rx.read_message()).await? {
        Ok(msg) => msg,
        Err(e) => return Err(PeerError::NetworkError { error: e.into(), message: "No response to connection message was received" })
    };
    if let Ok(connection_message) = ConnectionMessage::from_bytes(received_connection_msg.content().to_vec()) {
        let _ = storage.store_connection_message(&connection_message, true, addr);

        if !connection_message.versions.iter().any(|version| supported_protocol_version.supports(version)) {
            return Err(
                PeerError::UnsupportedProtocol {
                    supported_version: format!("{:?}", &supported_protocol_version),
                    incompatible_versions: format!("{:?}", &connection_message.versions()),
                }
            );
        }
    }

    // generate local and remote nonce
    let NoncePair { local: nonce_local, remote: nonce_remote } = generate_nonces(&connection_message_sent, &received_connection_msg, msg.incoming);

    // convert received bytes from remote peer into `ConnectionMessage`
    let received_connection_msg: ConnectionMessage = ConnectionMessage::try_from(received_connection_msg)?;
    let peer_public_key = received_connection_msg.public_key();
    let peer_id = HashType::CryptoboxPublicKeyHash.bytes_to_string(&peer_public_key);
    debug!(log, "Received peer public key"; "public_key" => &peer_id);

    // pre-compute encryption key
    let precomputed_key = match precompute(&hex::encode(peer_public_key), &info.secret_key) {
        Ok(key) => key,
        Err(_) => return Err(PeerError::FailedToPrecomputeKey)
    };

    // from now on all messages will be encrypted
    let mut msg_tx = EncryptedMessageWriter::new(msg_tx, precomputed_key.clone(), nonce_local, peer_id.clone(), log.clone());
    let mut msg_rx = EncryptedMessageReader::new(msg_rx, precomputed_key, nonce_remote, peer_id, log.clone());

    // send metadata
    let metadata = MetadataMessage::new(false, false);
    let _ = storage.store_metadata_message(&metadata, false, addr);
    timeout(IO_TIMEOUT, msg_tx.write_message(&metadata)).await??;

    // receive metadata
    let metadata_received = timeout(IO_TIMEOUT, msg_rx.read_message::<MetadataMessage>()).await??;
    let _ = storage.store_metadata_message(&metadata_received, true, addr);
    debug!(log, "Received remote peer metadata"; "disable_mempool" => metadata_received.disable_mempool(), "private_node" => metadata_received.private_node());

    // send ack
    timeout(IO_TIMEOUT, msg_tx.write_message(&AckMessage::Ack)).await??;

    // receive ack
    let ack_received = timeout(IO_TIMEOUT, msg_rx.read_message()).await??;

    match ack_received {
        AckMessage::Ack => {
            debug!(log, "Received ACK");
            Ok(BootstrapOutput(msg_rx, msg_tx, peer_public_key.clone()))
        }
        AckMessage::NackV0 => {
            debug!(log, "Received NACK");
            Err(PeerError::NackReceived)
        }
        AckMessage::Nack(nack_info) => {
            debug!(log, "Received NACK with info: {:?}", nack_info);
            Err(PeerError::NackWithMotiveReceived { nack_info })
        }
    }
}


/// Generate nonces (sent and recv encoding must be with length bytes also)
///
/// local_nonce is used for writing crypto messages to other peers
/// remote_nonce is used for reading crypto messages from other peers
fn generate_nonces(sent_msg: &BinaryChunk, recv_msg: &BinaryChunk, incoming: bool) -> NoncePair {
    nonce::generate_nonces(sent_msg.raw(), recv_msg.raw(), incoming)
}

/// Start to process incoming data
async fn begin_process_incoming(mut rx: EncryptedMessageReader, rx_run: Arc<AtomicBool>, myself: PeerRef, event_channel: NetworkChannelRef, log: Logger, mut storage: P2PMessageStorage, peer_addr: SocketAddr) {
    info!(log, "Starting to accept messages");

    while rx_run.load(Ordering::Acquire) {
        match timeout(READ_TIMEOUT_LONG, rx.read_message::<PeerMessageResponse>()).await {
            Ok(res) => match res {
                Ok(msg) => {
                    let _ = storage.store_peer_message(msg.messages(), false, peer_addr);
                    let should_broadcast_message = rx_run.load(Ordering::Acquire);
                    if should_broadcast_message {
                        trace!(log, "Message parsed successfully");
                        event_channel.tell(
                            Publish {
                                msg: PeerMessageReceived {
                                    peer: myself.clone(),
                                    message: Arc::new(msg),
                                }.into(),
                                topic: NetworkChannelTopic::NetworkEvents.into(),
                            }, Some(myself.clone().into()));
                    }
                }
                Err(e) => {
                    if let StreamError::DeserializationError { error: BinaryReaderError::UnsupportedTag { .. } } = e {
                        info!(log, "Messages with unsupported tags are ignored");
                    } else {
                        warn!(log, "Failed to read peer message"; "reason" => e);
                        break;
                    }
                }
            }
            Err(_) => {
                warn!(log, "Peer message read timed out"; "secs" => READ_TIMEOUT_LONG.as_secs());
                break;
            }
        }
    }
}
