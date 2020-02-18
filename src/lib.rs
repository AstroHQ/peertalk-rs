//! Crate to handle establishing network connections over USB to apple devices
#![forbid(missing_docs)]

use std::error::Error as StdError;
use std::fmt;
#[cfg(target_os = "windows")]
use std::net::TcpStream;
#[cfg(not(target_os = "windows"))]
use std::os::unix::net::UnixStream;

#[cfg(target_os = "windows")]
const WINDOWS_TCP_PORT: u16 = 27015;

mod protocol;
pub use protocol::{DeviceAttachedInfo, DeviceConnectionType, DeviceEvent, ProtocolError};
use protocol::{Packet, PacketType, Protocol};

/// Error for device listener etc
#[derive(Debug)]
pub enum Error {
    /// Error with usbmuxd protocol
    ProtocolError(protocol::ProtocolError),
    /// usbmuxd or Apple Mobile Service isn't available or installed
    ServiceUnavailable(std::io::Error),
    /// Error when registrering for device events failed
    FailedToListen(i64),
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::ProtocolError(e) => write!(f, "Protocol error: {}", e),
            Error::ServiceUnavailable(e) => write!(
                f,
                "Apple Mobile Device service (usbmuxd) likely not available: {}",
                e
            ),
            Error::FailedToListen(c) => write!(f, "Error registering device listener: code {}", c),
        }
    }
}
impl StdError for Error {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            Error::ProtocolError(e) => Some(e),
            Error::ServiceUnavailable(e) => Some(e),
            _ => None,
        }
    }
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::ServiceUnavailable(e)
    }
}
impl From<protocol::ProtocolError> for Error {
    fn from(e: protocol::ProtocolError) -> Self {
        Error::ProtocolError(e)
    }
}

/// Alias for any of this crate's results
pub type Result<T> = ::std::result::Result<T, Error>;

/// Listens for iOS devices connecting over USB via Apple Mobile Support/usbmuxd
pub struct DeviceListener {
    #[cfg(target_os = "windows")]
    socket: TcpStream,
    #[cfg(not(target_os = "windows"))]
    socket: UnixStream,
}
impl DeviceListener {
    /// Connects to usbmuxd (linux oss lib or macOS's built-in muxer)
    #[cfg(not(target_os = "windows"))]
    fn connect_unix() -> Result<UnixStream> {
        Ok(UnixStream::connect("/var/run/usbmuxd")?)
    }
    /// Connect's to Apple Mobile Support service on Windows if available (TCP 27015)
    #[cfg(target_os = "windows")]
    fn connect_windows() -> TcpStream {
        use std::net::{Ipv4Addr, SocketAddr};
        let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), WINDOWS_TCP_PORT);
        Ok(TcpStream::connect_timeout(
            &addr,
            std::time::Duration::from_secs(5),
        )?)
    }
    /// Produces a new device listener, registering with usbmuxd/apple mobile support service
    pub fn new() -> Result<Self> {
        #[cfg(target_os = "windows")]
        let socket = Self::connect_windows()?;
        #[cfg(not(target_os = "windows"))]
        let socket = Self::connect_unix()?;
        let mut listener = DeviceListener { socket };
        listener.start_listen()?;
        Ok(listener)
    }
    /// Receives an event, blocking until there is
    pub fn recv_event(&mut self) -> Result<DeviceEvent> {
        let packet = Packet::from_reader(&mut self.socket).unwrap();
        // println!("Read: {:?}", packet);
        if packet.protocol == Protocol::Plist {
            // let cursor = std::io::Cursor::new(&packet.data[..]);
            // let msg = protocol::DeviceEventMessage::from_reader(cursor);
            // println!("Payload message: {:?}", msg);
            Ok(DeviceEvent::from_vec(packet.data)?)
        } else {
            println!("Failed to get plist protocol message, ignoring");
            Err(Error::ProtocolError(ProtocolError::InvalidProtocol(0)))
        }
    }
    fn start_listen(&mut self) -> Result<()> {
        let command = protocol::Command::new("Listen");
        let mut payload: Vec<u8> = Vec::new();
        plist::to_writer_xml(&mut payload, &command).unwrap();
        assert_ne!(payload.len(), 0, "Should have > 0 bytes payload");
        self.send_payload(PacketType::PlistPayload, Protocol::Plist, payload);
        let packet = Packet::from_reader(&mut self.socket)?;
        let cursor = std::io::Cursor::new(&packet.data[..]);
        let res = protocol::ResultMessage::from_reader(cursor)?;
        if res.0 != 0 {
            return Err(Error::FailedToListen(res.0));
        }
        Ok(())
    }
    fn send_payload(&mut self, packet_type: PacketType, protocol: Protocol, payload: Vec<u8>) {
        let packet = Packet::new(protocol, packet_type, 0, payload);
        packet.write_into(&mut self.socket).unwrap();
    }
}
