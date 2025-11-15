//! Protocol buffer definitions for Eternal Terminal
//!
//! This crate contains the protocol buffer definitions used for communication
//! between ET clients and servers.

// Include the generated protobuf code
pub mod et {
    include!(concat!(env!("OUT_DIR"), "/et.rs"));
}

// Re-export commonly used types for convenience
pub use et::{
    ConnectRequest, ConnectResponse, ConnectStatus, EtPacketType, CatchupBuffer,
    SequenceHeader, SocketEndpoint,
};

pub use et::{
    TerminalPacketType, TerminalBuffer, TerminalInfo, PortForwardSourceRequest,
    PortForwardSourceResponse, PortForwardDestinationRequest,
    PortForwardDestinationResponse, PortForwardData, InitialPayload,
    InitialResponse, ConfigParams, TermInit, TerminalUserInfo,
};

// Re-export prost types for convenience
pub use prost::Message;
