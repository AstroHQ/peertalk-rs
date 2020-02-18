use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use serde::{Deserialize, Serialize};
use std::convert::TryFrom;
use std::fmt;
use std::io::{Read, Write};
use std::mem::size_of;
use std::net::TcpStream;

mod protocol;

#[derive(Debug)]
pub enum Error {
    InvalidPacketType(u32),
    InvalidProtocol(u32),
    InvalidReplyCode(u32),
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::InvalidPacketType(code) => write!(f, "Invalid Packet Type: {}", code),
            Error::InvalidProtocol(code) => write!(f, "Invalid Protocol: {}", code),
            Error::InvalidReplyCode(code) => write!(f, "Invalid Reply code: {}", code),
        }
    }
}
impl std::error::Error for Error {
    // fn source(&self) -> Option<&(dyn Error + 'static)> { }
}

pub type Result<T> = ::std::result::Result<T, Error>;

const BASE_PACKET_SIZE: u32 = size_of::<u32>() as u32 * 4;
// const USB_MESSAGE_TYPE_KEY: &str = "MessageType";
// const USB_DEVICE_ID_KEY: &str = "DeviceID";
// const USB_DEVICE_PROPERTIES_KEY: &str = "Properties";

#[repr(u32)]
#[derive(Copy, Clone, Debug, PartialEq)]
enum PacketType {
    Result = 1,
    Connect = 2,
    Listen = 3,
    DeviceAdd = 4,
    DeviceRemove = 5,
    // 6 unknown
    // 7 unknown
    PlistPayload = 8,
}
impl Into<u32> for PacketType {
    fn into(self) -> u32 {
        self as u32
    }
}

impl TryFrom<u32> for PacketType {
    type Error = crate::Error;
    fn try_from(value: u32) -> Result<Self> {
        match value {
            1 => Ok(Self::Result),
            2 => Ok(Self::Connect),
            3 => Ok(Self::Listen),
            4 => Ok(Self::DeviceAdd),
            5 => Ok(Self::DeviceRemove),
            8 => Ok(Self::PlistPayload),
            c => Err(Error::InvalidPacketType(c)),
        }
    }
}
#[repr(u32)]
#[derive(Copy, Clone, Debug, PartialEq)]
enum Protocol {
    Binary = 0,
    Plist = 1,
}
impl Into<u32> for Protocol {
    fn into(self) -> u32 {
        self as u32
    }
}
impl TryFrom<u32> for Protocol {
    type Error = crate::Error;
    fn try_from(value: u32) -> Result<Self> {
        match value {
            0 => Ok(Protocol::Binary),
            1 => Ok(Protocol::Plist),
            c => Err(Error::InvalidProtocol(c)),
        }
    }
}

#[repr(u32)]
#[derive(Copy, Clone, Debug, PartialEq)]
enum ReplyCode {
    Ok = 0,
    BadCommand = 1,
    BadDevice = 2,
    ConnectionRefused = 3,
    // 4 unknown
    // 5 unknown
    BadVersion = 6,
}
impl Into<u32> for ReplyCode {
    fn into(self) -> u32 {
        self as u32
    }
}
impl TryFrom<u32> for ReplyCode {
    type Error = crate::Error;
    fn try_from(value: u32) -> Result<Self> {
        match value {
            0 => Ok(ReplyCode::Ok),
            1 => Ok(ReplyCode::BadCommand),
            2 => Ok(ReplyCode::BadDevice),
            3 => Ok(ReplyCode::ConnectionRefused),
            6 => Ok(ReplyCode::BadVersion),
            c => Err(Error::InvalidReplyCode(c)),
        }
    }
}
struct Packet {
    size: u32,
    protocol: Protocol,
    packet_type: PacketType,
    tag: u32,
    data: Vec<u8>,
}
impl fmt::Debug for Packet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Packet {{ size: {}, protocol: {:?}, packet_type: {:?}, tag: {}, payload(bytes): {} }}",
            self.size,
            self.protocol,
            self.packet_type,
            self.tag,
            self.data.len()
        )
    }
}
impl Packet {
    fn new(protocol: Protocol, packet_type: PacketType, tag: u32, payload: Vec<u8>) -> Self {
        assert!(
            payload.len() < u32::max_value() as usize,
            "Payload too large"
        );
        Packet {
            size: BASE_PACKET_SIZE + payload.len() as u32,
            protocol,
            packet_type,
            tag,
            data: payload,
        }
    }
    pub fn write_into<W>(&self, writer: &mut W) -> Result<()>
    where
        W: Write,
    {
        writer.write_u32::<LittleEndian>(self.size).unwrap();
        writer
            .write_u32::<LittleEndian>(self.protocol as u32)
            .unwrap();
        writer
            .write_u32::<LittleEndian>(self.packet_type.into())
            .unwrap();
        writer.write_u32::<LittleEndian>(self.tag).unwrap();
        writer.write_all(&self.data).unwrap();
        Ok(())
    }
    pub fn from_reader<R>(reader: &mut R) -> Result<Self>
    where
        R: Read,
    {
        let size = reader.read_u32::<LittleEndian>().unwrap();
        let protocol = Protocol::try_from(reader.read_u32::<LittleEndian>().unwrap())?;
        let packet_type = PacketType::try_from(reader.read_u32::<LittleEndian>().unwrap())?;
        let tag = reader.read_u32::<LittleEndian>().unwrap();
        let payload_size = size - BASE_PACKET_SIZE; // get what's left
        let data = if payload_size > 0 {
            let mut payload = vec![0; payload_size as usize];
            reader.read_exact(&mut payload).unwrap();
            payload
        } else {
            vec![]
        };
        let mut packet = Packet::new(protocol, packet_type, tag, data);
        packet.size = size;
        Ok(packet)
    }
}

#[derive(Serialize, Deserialize)]
struct Command {
    #[serde(rename = "MessageType")]
    message_type: String,
    #[serde(rename = "ProgName")]
    prog_name: String,
    #[serde(rename = "ClientVersionString")]
    client_version_string: String,
    // args: HashMap<String, String>,
}
impl Command {
    fn new<C: AsRef<str>>(command: C) -> Self {
        Command {
            message_type: command.as_ref().to_owned(),
            prog_name: String::from("MyApp"),
            client_version_string: String::from("1.0"),
        }
    }
}
pub struct DeviceListener {
    socket: TcpStream,
}
impl DeviceListener {
    pub fn new() -> Self {
        use std::net::SocketAddr;
        let addr: SocketAddr = "127.0.0.1:27015".parse().unwrap();
        let socket = TcpStream::connect_timeout(&addr, std::time::Duration::from_secs(5))
            .expect("Failed to connect to USB mux service");
        let mut listener = DeviceListener { socket };
        listener.start_listen();
        listener
    }
    fn start_listen(&mut self) {
        let command = Command::new("Listen");
        let mut payload: Vec<u8> = Vec::new();
        plist::to_writer_xml(&mut payload, &command).unwrap();
        assert_ne!(payload.len(), 0, "Should have > 0 bytes payload");
        self.send_payload(PacketType::PlistPayload, Protocol::Plist, payload);
        println!("Payload sent, waiting for response...");
        let packet = Packet::from_reader(&mut self.socket).unwrap();
        // let result = protocol::ResultMessage::from(0);
        let cursor = std::io::Cursor::new(&packet.data[..]);
        let res = protocol::ResultMessage::from_reader(cursor);
        println!("Got result: {:?}", res);
        loop {
            let packet = Packet::from_reader(&mut self.socket).unwrap();
            // println!("Read: {:?}", packet);
            if packet.protocol == Protocol::Plist {
                let cursor = std::io::Cursor::new(&packet.data[..]);
                let msg = protocol::DeviceEventMessage::from_reader(cursor);
                println!("Payload message: {:?}", msg);
            } else {
                println!("Failed to get plist protocol message, ignoring");
            }

            // let s = std::str::from_utf8(&packet.data[..]).unwrap();
            // println!("Payload: {}", s);
        }
    }
    fn send_payload(&mut self, packet_type: PacketType, protocol: Protocol, payload: Vec<u8>) {
        let packet = Packet::new(protocol, packet_type, 0, payload);
        packet.write_into(&mut self.socket).unwrap();
    }
}
/*
// Shared dictionary
MessageType => command,
ProgName => app bundle name,
ClientVersionString => bundle version,
// payload args below

// Listen
MessageType => Listen,

*/

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn it_works() {
        let command = Command::new("Listen");
        plist::to_file_xml("test.plist", &command).unwrap();
    }
}
