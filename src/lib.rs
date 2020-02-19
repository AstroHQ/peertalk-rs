//! Crate to handle establishing network connections over USB to apple devices
#![forbid(missing_docs)]

use std::collections::VecDeque;
use std::error::Error as StdError;
use std::fmt;
#[cfg(target_os = "windows")]
use std::net::TcpStream;
#[cfg(not(target_os = "windows"))]
use std::os::unix::net::UnixStream;

#[cfg(target_os = "windows")]
const WINDOWS_TCP_PORT: u16 = 27015;

mod protocol;
pub use protocol::{
    DeviceAttachedInfo, DeviceConnectionType, DeviceEvent, DeviceId, ProtocolError,
};
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
    /// Error establishing network connection to device
    ConnectionRefused(i64),
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
            Error::ConnectionRefused(c) => write!(f, "Error connecting to device: {}", c),
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

/// Aliases UsbSocket to std::net::TcpStream on Windows
#[cfg(target_os = "windows")]
pub type UsbSocket = TcpStream;
/// Aliases UsbSocket to std::os::unix::net::UnixStream on linux/macOS
#[cfg(not(target_os = "windows"))]
pub type UsbSocket = UnixStream;

/// Connects to usbmuxd (linux oss lib or macOS's built-in muxer)
#[cfg(not(target_os = "windows"))]
fn connect_unix() -> Result<UsbSocket> {
    Ok(UnixStream::connect("/var/run/usbmuxd")?)
}
/// Connect's to Apple Mobile Support service on Windows if available (TCP 27015)
#[cfg(target_os = "windows")]
fn connect_windows() -> Result<UsbSocket> {
    use std::net::{IpAddr, Ipv4Addr, SocketAddr};
    let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), WINDOWS_TCP_PORT);
    Ok(TcpStream::connect_timeout(
        &addr,
        std::time::Duration::from_secs(5),
    )?)
}

fn send_payload(
    socket: &mut UsbSocket,
    packet_type: PacketType,
    protocol: Protocol,
    payload: Vec<u8>,
) -> Result<()> {
    let packet = Packet::new(protocol, packet_type, 0, payload);
    Ok(packet.write_into(socket)?)
}
/// Creates a network connection over USB to given device & port
pub fn connect_to_device(device_id: protocol::DeviceId, port: u16) -> Result<UsbSocket> {
    #[cfg(target_os = "windows")]
    let mut socket = connect_windows()?;
    #[cfg(not(target_os = "windows"))]
    let mut socket = connect_unix()?;
    let command = protocol::Command::connect(port, device_id);
    let payload = command.to_bytes();
    send_payload(
        &mut socket,
        PacketType::PlistPayload,
        Protocol::Plist,
        payload,
    )?;
    let packet = Packet::from_reader(&mut socket)?;
    let cursor = std::io::Cursor::new(&packet.data[..]);
    let res = protocol::ResultMessage::from_reader(cursor)?;
    if res.0 != 0 {
        return Err(Error::ConnectionRefused(res.0));
    }

    Ok(socket)
}
/// Listens for iOS devices connecting over USB via Apple Mobile Support/usbmuxd
pub struct DeviceListener {
    #[cfg(target_os = "windows")]
    socket: TcpStream,
    #[cfg(not(target_os = "windows"))]
    socket: UnixStream,
    events: VecDeque<DeviceEvent>,
}
impl DeviceListener {
    /// Produces a new device listener, registering with usbmuxd/apple mobile support service
    pub fn new() -> Result<Self> {
        #[cfg(target_os = "windows")]
        let socket = connect_windows()?;
        #[cfg(not(target_os = "windows"))]
        let socket = connect_unix()?;
        let mut listener = DeviceListener {
            socket,
            events: VecDeque::new(),
        };
        listener.start_listen()?;
        listener.socket.set_nonblocking(true)?;
        Ok(listener)
    }
    /// Receives an event, blocking until there is
    pub fn next_event(&mut self) -> Option<DeviceEvent> {
        self.drain_events();
        self.events.pop_front()
    }
    fn drain_events(&mut self) {
        loop {
            match Packet::from_reader(&mut self.socket) {
                Ok(packet) => {
                    let msg = DeviceEvent::from_vec(packet.data).unwrap();
                    self.events.push_back(msg);
                }
                Err(ProtocolError::IoError(e)) => match e.kind() {
                    std::io::ErrorKind::WouldBlock => {
                        break;
                    }
                    _ => {
                        println!("IO Error: {}", e);
                        break;
                    }
                },
                Err(e) => {
                    println!("Error receiving events: {}", e);
                    break;
                }
            }
        }
    }
    fn start_listen(&mut self) -> Result<()> {
        let command = protocol::Command::listen();
        let payload = command.to_bytes();
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
