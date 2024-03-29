//! Crate to handle establishing network connections over USB to apple devices
#![forbid(missing_docs)]
use std::cell::RefCell;

#[macro_use]
extern crate log;

use std::collections::VecDeque;
#[cfg(target_os = "windows")]
use std::net::TcpStream;
#[cfg(not(target_os = "windows"))]
use std::os::unix::net::UnixStream;

#[cfg(target_os = "windows")]
const WINDOWS_TCP_PORT: u16 = 27015;

mod protocol;
pub use protocol::{
    DeviceAttachedInfo, DeviceConnectionType, DeviceEvent, DeviceId, ProductType, ProtocolError,
};
use protocol::{Packet, PacketType, Protocol};

/// Error for device listener etc
#[derive(thiserror::Error, Debug)]
pub enum Error {
    /// Error with usbmuxd protocol
    #[error("protocol error: {0}")]
    ProtocolError(#[from] protocol::ProtocolError),
    /// usbmuxd or Apple Mobile Service isn't available or installed
    #[error("Apple Mobile Device service (usbmuxd) likely not available: {0}")]
    ServiceUnavailable(#[from] std::io::Error),
    /// Error when registrering for device events failed
    #[error("error registering device listener: code {0}")]
    FailedToListen(i64),
    /// Error establishing network connection to device
    #[error("error connecting to device: {0}")]
    ConnectionRefused(i64),
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
    socket: RefCell<TcpStream>,
    #[cfg(not(target_os = "windows"))]
    socket: RefCell<UnixStream>,
    events: RefCell<VecDeque<DeviceEvent>>,
}
impl DeviceListener {
    /// Produces a new device listener, registering with usbmuxd/apple mobile support service
    ///
    /// # Errors
    /// Can produce an error, most commonly when the mobile service isn't available. It should be available on macOS,
    /// but on Windows it's only available if Apple Mobile Support is installed, typically via iTunes.
    pub fn new() -> Result<Self> {
        #[cfg(target_os = "windows")]
        let socket = connect_windows()?;
        #[cfg(not(target_os = "windows"))]
        let socket = connect_unix()?;
        let listener = DeviceListener {
            socket: RefCell::new(socket),
            events: RefCell::new(VecDeque::new()),
        };
        listener.start_listen()?;
        listener.socket.borrow_mut().set_nonblocking(true)?;
        Ok(listener)
    }
    /// Receives an event, None if there's no pending events at this time
    pub fn next_event(&self) -> Option<DeviceEvent> {
        self.drain_events();
        self.events.borrow_mut().pop_front()
    }
    fn drain_events(&self) {
        // TODO: better way read on demand? maybe just thread it?
        use std::io::Read;
        let mut retries_left = 5;
        let mut data: Vec<u8> = Vec::with_capacity(10_000);
        let full_data = loop {
            let mut buf = [0; 4096];
            match (*self.socket.borrow_mut()).read(&mut buf) {
                Ok(bytes) => {
                    data.extend_from_slice(&buf[0..bytes]);
                }
                Err(e) => {
                    if e.kind() == std::io::ErrorKind::WouldBlock {
                        retries_left -= 1;
                        std::thread::sleep(std::time::Duration::from_millis(100));
                    }
                }
            }
            if retries_left == 0 {
                break data;
            }
        };
        let mut cursor = std::io::Cursor::new(&full_data[..]);
        loop {
            if cursor.position() == full_data.len() as u64 {
                break;
            }
            match Packet::from_reader(&mut cursor) {
                Ok(packet) => {
                    let msg = DeviceEvent::from_vec(packet.data).unwrap();
                    self.events.borrow_mut().push_back(msg);
                }
                Err(ProtocolError::IoError(e)) => match e.kind() {
                    std::io::ErrorKind::WouldBlock => {
                        break;
                    }
                    _ => {
                        error!("IO Error: {}", e);
                        break;
                    }
                },
                Err(e) => {
                    error!("Error receiving events: {}", e);
                    break;
                }
            }
        }
    }
    fn start_listen(&self) -> Result<()> {
        info!("Starting device listen");
        let command = protocol::Command::listen();
        let payload = command.to_bytes();
        send_payload(
            &mut self.socket.borrow_mut(),
            PacketType::PlistPayload,
            Protocol::Plist,
            payload,
        )?;
        let packet = Packet::from_reader(&mut *self.socket.borrow_mut())?;
        let cursor = std::io::Cursor::new(&packet.data[..]);
        let res = protocol::ResultMessage::from_reader(cursor)?;
        if res.0 != 0 {
            error!("Failed to setup device listen: {}", res.0);
            return Err(Error::FailedToListen(res.0));
        }
        info!("Listen successful");
        Ok(())
    }
}
