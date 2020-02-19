// use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use plist::Value;
use serde::{Deserialize, Serialize};
use std::convert::TryFrom;
use std::error::Error;
use std::fmt;
use std::io::{Error as IoError, Read, Seek, Write};
use std::mem::size_of;

/// Error type for any errors with talking to USB muxer/device support
#[derive(Debug)]
pub enum ProtocolError {
    /// Message type is invalid, or unsupported
    InvalidMessageType(String),
    /// Plist entry isn't the type expected
    InvalidPlistEntry,
    /// Plist entry for key is invalid/wrong type
    InvalidPlistEntryForKey(&'static str),
    /// Invalid packet type value
    InvalidPacketType(u32),
    /// Invalid protocol value (expect 0 or 1)
    InvalidProtocol(u32),
    /// Invalid reply code (expect 0-6 except 4, 5)
    InvalidReplyCode(u32),
    /// An IO error occurred, usually if reading from file/socket
    IoError(IoError),
}
impl fmt::Display for ProtocolError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ProtocolError::InvalidMessageType(t) => write!(f, "Invalid message type: {}", t),
            ProtocolError::InvalidPlistEntry => write!(f, "Invalid plist format/entry"),
            ProtocolError::InvalidPlistEntryForKey(key) => {
                write!(f, "Invalid plist entry for key: {}", key)
            }
            ProtocolError::InvalidPacketType(code) => write!(f, "Invalid Packet Type: {}", code),
            ProtocolError::InvalidProtocol(code) => write!(f, "Invalid Protocol: {}", code),
            ProtocolError::InvalidReplyCode(code) => write!(f, "Invalid Reply code: {}", code),
            ProtocolError::IoError(e) => write!(f, "IoError: {}", e),
        }
    }
}
impl Error for ProtocolError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            ProtocolError::IoError(e) => Some(e),
            _ => None,
        }
    }
}
impl From<IoError> for ProtocolError {
    fn from(error: IoError) -> Self {
        ProtocolError::IoError(error)
    }
}

/// Result type
pub type Result<T> = ::std::result::Result<T, ProtocolError>;

const BASE_PACKET_SIZE: u32 = size_of::<u32>() as u32 * 4;
const USB_MESSAGE_TYPE_KEY: &str = "MessageType";
const USB_DEVICE_ID_KEY: &str = "DeviceID";
const USB_DEVICE_PROPERTIES_KEY: &str = "Properties";

#[repr(u32)]
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum PacketType {
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
    type Error = ProtocolError;
    fn try_from(value: u32) -> Result<Self> {
        match value {
            1 => Ok(Self::Result),
            2 => Ok(Self::Connect),
            3 => Ok(Self::Listen),
            4 => Ok(Self::DeviceAdd),
            5 => Ok(Self::DeviceRemove),
            8 => Ok(Self::PlistPayload),
            c => Err(ProtocolError::InvalidPacketType(c)),
        }
    }
}
#[repr(u32)]
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Protocol {
    Binary = 0,
    Plist = 1,
}
impl Into<u32> for Protocol {
    fn into(self) -> u32 {
        self as u32
    }
}
impl TryFrom<u32> for Protocol {
    type Error = ProtocolError;
    fn try_from(value: u32) -> Result<Self> {
        match value {
            0 => Ok(Protocol::Binary),
            1 => Ok(Protocol::Plist),
            c => Err(ProtocolError::InvalidProtocol(c)),
        }
    }
}

#[repr(u32)]
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum ReplyCode {
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
    type Error = ProtocolError;
    fn try_from(value: u32) -> Result<Self> {
        match value {
            0 => Ok(ReplyCode::Ok),
            1 => Ok(ReplyCode::BadCommand),
            2 => Ok(ReplyCode::BadDevice),
            3 => Ok(ReplyCode::ConnectionRefused),
            6 => Ok(ReplyCode::BadVersion),
            c => Err(ProtocolError::InvalidReplyCode(c)),
        }
    }
}
pub struct Packet {
    pub size: u32,
    pub protocol: Protocol,
    pub packet_type: PacketType,
    pub tag: u32,
    pub data: Vec<u8>,
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
    pub fn new(protocol: Protocol, packet_type: PacketType, tag: u32, payload: Vec<u8>) -> Self {
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
        let size = reader.read_u32::<LittleEndian>()?;
        let protocol = Protocol::try_from(reader.read_u32::<LittleEndian>()?)?;
        let packet_type = PacketType::try_from(reader.read_u32::<LittleEndian>()?)?;
        let tag = reader.read_u32::<LittleEndian>()?;
        let payload_size = size - BASE_PACKET_SIZE; // get what's left
        let data = if payload_size > 0 {
            let mut payload = vec![0; payload_size as usize];
            reader.read_exact(&mut payload)?;
            payload
        } else {
            vec![]
        };
        let mut packet = Packet::new(protocol, packet_type, tag, data);
        packet.size = size;
        Ok(packet)
    }
}

#[derive(Debug, PartialEq, Copy, Clone)]
pub enum MessageType {
    Paired,
    Result,
    Detached,
    Attached,
}
impl TryFrom<&Value> for MessageType {
    type Error = ProtocolError;
    fn try_from(value: &Value) -> Result<Self> {
        match value {
            Value::String(s) => match s.as_str() {
                "Paired" => Ok(MessageType::Paired),
                "Result" => Ok(MessageType::Result),
                "Attached" => Ok(MessageType::Attached),
                "Detached" => Ok(MessageType::Detached),
                s => Err(ProtocolError::InvalidMessageType(s.to_owned())),
            },
            _ => Err(ProtocolError::InvalidMessageType(
                "Invalid PLIST type".to_owned(),
            )),
        }
    }
}

/// Device ID type, currently u64 to hold max value stored in plist
pub type DeviceId = u64;
/// Product type of connected device, which typically is an iPad, iPhone, or iPod touch
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ProductType {
    /// Any iPhone that's connected
    IPhone,
    /// iPod touch
    IPodTouch,
    /// iPad/iPad Pro
    IPad,
    /// Unexpected product id we haven't coded for yet
    Unknown(u16),
}
impl From<u16> for ProductType {
    fn from(product_id: u16) -> Self {
        match product_id {
            0x12A8 => ProductType::IPhone,
            0x12AA => ProductType::IPodTouch,
            0x12AB => ProductType::IPad,
            p => ProductType::Unknown(p),
        }
    }
}
/// How device is connected
#[derive(Debug, PartialEq)]
pub enum DeviceConnectionType {
    /// USB connection type
    USB,
    /// Wi-fi maybe? have yet to see it
    Unknown(String),
}
impl TryFrom<&Value> for DeviceConnectionType {
    type Error = ProtocolError;
    fn try_from(value: &Value) -> Result<Self> {
        match value.as_string() {
            Some("USB") => Ok(DeviceConnectionType::USB),
            Some(s) => Ok(DeviceConnectionType::Unknown(s.to_owned())),
            None => Err(ProtocolError::InvalidPlistEntryForKey("ConnectionType")),
        }
    }
}
/// Info about an attached device
#[derive(Debug)]
pub struct DeviceAttachedInfo {
    /// Type of connection device is using (USB or otherwise)
    pub connection_type: DeviceConnectionType,
    /// ID of device
    pub device_id: DeviceId,
    /// Unknown purpose/value
    pub location_id: u64,
    /// Product type of device, ipad, ipod, iphone, mysterious other device
    pub product_type: ProductType,
    /// Device's identifier/serial
    pub identifier: String,
}
// TODO: this likely could be done from within serde maybe? custom deserialization?
impl TryFrom<&Value> for DeviceAttachedInfo {
    type Error = ProtocolError;
    fn try_from(value: &Value) -> Result<Self> {
        match value {
            Value::Dictionary(d) => {
                let connection_type = d
                    .get("ConnectionType")
                    .and_then(|t| DeviceConnectionType::try_from(t).ok())
                    .ok_or(ProtocolError::InvalidPlistEntryForKey("ConnectionType"))?;
                let device_id = d
                    .get(USB_DEVICE_ID_KEY)
                    .and_then(Value::as_unsigned_integer)
                    .ok_or(ProtocolError::InvalidPlistEntryForKey(USB_DEVICE_ID_KEY))?;
                let location_id = d
                    .get("LocationID")
                    .and_then(Value::as_unsigned_integer)
                    .ok_or(ProtocolError::InvalidPlistEntryForKey("LocationID"))?;
                let product_type = d
                    .get("ProductID")
                    .and_then(Value::as_unsigned_integer)
                    .and_then(|i| Some(ProductType::from(i as u16))) // product_id is USB product_id which is u16
                    .ok_or(ProtocolError::InvalidPlistEntryForKey("ProductID"))?;
                let identifier = d
                    .get("SerialNumber")
                    .and_then(Value::as_string)
                    .ok_or(ProtocolError::InvalidPlistEntryForKey("SerialNumber"))?
                    .to_owned();
                Ok(DeviceAttachedInfo {
                    connection_type,
                    device_id,
                    location_id,
                    product_type,
                    identifier,
                })
            }
            _ => Err(ProtocolError::InvalidPlistEntry),
        }
    }
}
#[derive(Debug)]
/// Event that can occur on device listener
pub enum DeviceEvent {
    /// Device was plugged into host
    Attached(DeviceAttachedInfo),
    /// Device was unplugged from host
    Detached(DeviceId),
    /// Device was paired to host (trusting computer was authorized)
    Paired(DeviceId),
}
impl TryFrom<&Value> for DeviceEvent {
    type Error = ProtocolError;
    fn try_from(value: &Value) -> Result<Self> {
        match value {
            Value::Dictionary(d) => {
                let msg_type = MessageType::try_from(d.get(USB_MESSAGE_TYPE_KEY).unwrap())?;
                let device_id = d
                    .get(USB_DEVICE_ID_KEY)
                    .and_then(Value::as_unsigned_integer)
                    .ok_or(ProtocolError::InvalidPlistEntryForKey(USB_DEVICE_ID_KEY))?;
                match msg_type {
                    MessageType::Attached => {
                        let device_info = d
                            .get(USB_DEVICE_PROPERTIES_KEY)
                            .and_then(|p| DeviceAttachedInfo::try_from(p).ok())
                            .ok_or(ProtocolError::InvalidPlistEntryForKey(
                                USB_DEVICE_PROPERTIES_KEY,
                            ))?;
                        Ok(DeviceEvent::Attached(device_info))
                    }
                    MessageType::Detached => Ok(DeviceEvent::Detached(device_id)),
                    MessageType::Paired => Ok(DeviceEvent::Paired(device_id)),
                    MessageType::Result => {
                        Err(ProtocolError::InvalidMessageType("Result".to_owned()))
                    }
                }
            }
            _ => Err(ProtocolError::InvalidPlistEntry),
        }
    }
}
impl DeviceEvent {
    pub(crate) fn from_vec(data: Vec<u8>) -> Result<DeviceEvent> {
        let cursor = std::io::Cursor::new(&data[..]);
        let dict: Value = Value::from_reader(cursor).unwrap();
        DeviceEvent::try_from(&dict)
    }
}

#[derive(Debug)]
pub struct ResultMessage(pub i64);
impl ResultMessage {
    pub fn from_reader<R: Read + Seek>(reader: R) -> Result<Self> {
        let r: plist::Value = plist::Value::from_reader(reader).unwrap();
        ResultMessage::try_from(&r)
    }
}
impl TryFrom<&Value> for ResultMessage {
    type Error = ProtocolError;
    fn try_from(value: &Value) -> Result<Self> {
        match value {
            Value::Dictionary(d) => {
                let num = d
                    .get("Number")
                    .and_then(Value::as_signed_integer)
                    .ok_or(ProtocolError::InvalidPlistEntryForKey("SerialNumber"))?;
                Ok(ResultMessage(num))
            }
            _ => Err(ProtocolError::InvalidPlistEntry),
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct Command {
    #[serde(rename = "MessageType")]
    message_type: String,
    #[serde(rename = "ProgName")]
    prog_name: String,
    #[serde(rename = "ClientVersionString")]
    client_version_string: String,
    #[serde(rename = "PortNumber")]
    port_number: Option<u16>,
    #[serde(rename = "DeviceID")]
    device_id: Option<DeviceId>,
}
impl Command {
    fn new<C: AsRef<str>>(command: C) -> Self {
        Command {
            message_type: command.as_ref().to_owned(),
            prog_name: String::from("Peertalk Example"),
            client_version_string: String::from("1"),
            port_number: None,
            device_id: None,
        }
    }
    pub fn listen() -> Self {
        Command::new("Listen")
    }
    pub fn connect(port: u16, device_id: DeviceId) -> Self {
        let mut command = Command::new("Connect");
        command.port_number = Some(port.to_be()); // apple's service expects network byte order
        command.device_id = Some(device_id);
        command
    }
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut payload: Vec<u8> = Vec::new();
        plist::to_writer_xml(&mut payload, &self).unwrap();
        assert_ne!(payload.len(), 0, "Should have > 0 bytes payload");
        payload
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn value_for_testfile(file: &str) -> plist::Value {
        let mut path = std::path::PathBuf::new();
        path.push("test_data");
        path.push(file);
        plist::Value::from_file(path).unwrap()
    }
    #[test]
    fn it_decodes_plists() {
        let r = value_for_testfile("detached.plist");
        match DeviceEvent::try_from(&r) {
            Ok(DeviceEvent::Detached(device_id)) => assert_eq!(device_id, 3),
            _ => assert!(false, "Invalid DeviceEvent"),
        }
        let r = value_for_testfile("paired.plist");
        match DeviceEvent::try_from(&r) {
            Ok(DeviceEvent::Paired(device_id)) => assert_eq!(device_id, 3),
            _ => assert!(false, "Invalid DeviceEvent"),
        }
        let r = value_for_testfile("success-result.plist");
        let msg = ResultMessage::try_from(&r);
        assert!(msg.is_ok());
        println!("Test: {:?}", msg);
    }
    #[test]
    fn it_decodes_attached() {
        let r = value_for_testfile("attached.plist");
        let msg = DeviceEvent::try_from(&r);
        assert!(msg.is_ok());
        match DeviceEvent::try_from(&r) {
            Ok(DeviceEvent::Attached(device_info)) => {
                assert_eq!(device_info.device_id, 3);
                assert_eq!(device_info.connection_type, DeviceConnectionType::USB);
                assert_eq!(device_info.location_id, 0);
                assert_eq!(device_info.product_type, ProductType::IPad);
                assert_eq!(device_info.identifier, "00001011-000A111E0111001E");
            }
            _ => assert!(false, "Invalid DeviceEvent"),
        }
    }

    #[test]
    fn it_decodes_command() {
        let command: Command = plist::from_file("test_data/command.plist").unwrap();
        assert_eq!(command.message_type, "Listen");
        assert_eq!(command.prog_name, "MyApp");
        assert_eq!(command.client_version_string, "1.0");
    }
    #[test]
    fn it_encodes_command() {
        let mut command = Command::new("Connect");
        command.port_number = Some(12345);
        command.device_id = Some(16689);
        plist::to_file_xml("test.plist", &command).unwrap();
    }
}
