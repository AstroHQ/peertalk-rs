// use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use plist::Value;
use serde::{Deserialize, Serialize};
use std::convert::TryFrom;
use std::error::Error;
use std::fmt;
use std::io::{Read, Seek, Write};
use std::mem::size_of;
/// Error type for any errors with talking to USB muxer/device support
#[derive(Debug)]
pub enum ProtocolError {
    InvalidMessageType(String),
    InvalidPlistEntry,
    InvalidPlistEntryForKey(&'static str),
    /// Invalid packet type value
    InvalidPacketType(u32),
    /// Invalid protocol value (expect 0 or 1)
    InvalidProtocol(u32),
    /// Invalid reply code (expect 0-6 except 4, 5)
    InvalidReplyCode(u32),
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
        }
    }
}
impl Error for ProtocolError {
    // fn source(&self) -> Option<&(dyn Error + 'static)> { }
}

/// Result type
pub type Result<T> = ::std::result::Result<T, ProtocolError>;

const BASE_PACKET_SIZE: u32 = size_of::<u32>() as u32 * 4;
// const USB_MESSAGE_TYPE_KEY: &str = "MessageType";
// const USB_DEVICE_ID_KEY: &str = "DeviceID";
// const USB_DEVICE_PROPERTIES_KEY: &str = "Properties";

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
/*
<key>ConnectionType</key>
<string>USB</string>
<key>DeviceID</key>
<integer>3</integer>
<key>LocationID</key>
<integer>0</integer>
<key>ProductID</key>
<integer>4779</integer>
<key>SerialNumber</key>
<string>00008027-000C486C0222002E</string>*/
#[derive(Debug)]
pub struct DeviceProperties {
    pub connection_type: String,
    pub device_id: u64,
    pub location_id: u64,
    pub product_id: u64,
    pub serial_number: String,
}
impl TryFrom<&Value> for DeviceProperties {
    type Error = ProtocolError;
    fn try_from(value: &Value) -> Result<Self> {
        match value {
            Value::Dictionary(d) => {
                let connection_type = d
                    .get("ConnectionType")
                    .and_then(Value::as_string)
                    .ok_or(ProtocolError::InvalidPlistEntryForKey("ConnectionType"))?
                    .to_owned();
                let device_id = d
                    .get("DeviceID")
                    .and_then(Value::as_unsigned_integer)
                    .ok_or(ProtocolError::InvalidPlistEntryForKey("DeviceID"))?;
                let location_id = d
                    .get("LocationID")
                    .and_then(Value::as_unsigned_integer)
                    .ok_or(ProtocolError::InvalidPlistEntryForKey("LocationID"))?;
                let product_id = d
                    .get("ProductID")
                    .and_then(Value::as_unsigned_integer)
                    .ok_or(ProtocolError::InvalidPlistEntryForKey("ProductID"))?;
                let serial_number = d
                    .get("SerialNumber")
                    .and_then(Value::as_string)
                    .ok_or(ProtocolError::InvalidPlistEntryForKey("SerialNumber"))?
                    .to_owned();
                Ok(DeviceProperties {
                    connection_type,
                    device_id,
                    location_id,
                    product_id,
                    serial_number,
                })
            }
            _ => Err(ProtocolError::InvalidPlistEntry),
        }
    }
}
#[derive(Debug)]
pub struct ResultMessage(i64);
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
#[derive(Debug)]
pub struct DeviceEventMessage {
    pub message_type: MessageType,
    pub device_id: u64,
    pub device_properties: Option<DeviceProperties>,
}

impl DeviceEventMessage {
    pub fn from_reader<R: Read + Seek>(reader: R) -> Result<Self> {
        let r: plist::Value = plist::Value::from_reader(reader).unwrap();
        DeviceEventMessage::try_from(&r)
    }
}
impl TryFrom<&Value> for DeviceEventMessage {
    type Error = ProtocolError;
    fn try_from(value: &Value) -> Result<Self> {
        match value {
            Value::Dictionary(d) => {
                let msg_type = MessageType::try_from(d.get("MessageType").unwrap())?;
                let device_id = d
                    .get("DeviceID")
                    .and_then(|int| int.as_unsigned_integer())
                    .ok_or(ProtocolError::InvalidPlistEntryForKey("DeviceID"))?;
                let device_properties = d
                    .get("Properties")
                    .and_then(|p| DeviceProperties::try_from(p).ok());
                Ok(DeviceEventMessage {
                    message_type: msg_type,
                    device_id,
                    device_properties,
                })
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
    // args: HashMap<String, String>,
}
impl Command {
    pub fn new<C: AsRef<str>>(command: C) -> Self {
        Command {
            message_type: command.as_ref().to_owned(),
            prog_name: String::from("MyApp"),
            client_version_string: String::from("1.0"),
        }
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
        let msg = DeviceEventMessage::try_from(&r);
        assert!(msg.is_ok());
        let msg = msg.unwrap();
        assert_eq!(msg.device_id, 3);
        assert_eq!(msg.message_type, MessageType::Detached);
        println!("Test: {:?}", msg);
        let r = value_for_testfile("paired.plist");
        let msg = DeviceEventMessage::try_from(&r);
        assert!(msg.is_ok());
        let msg = msg.unwrap();
        assert_eq!(msg.device_id, 3);
        assert_eq!(msg.message_type, MessageType::Paired);
        println!("Test: {:?}", msg);

        let r = value_for_testfile("success-result.plist");
        let msg = ResultMessage::try_from(&r);
        assert!(msg.is_ok());
        println!("Test: {:?}", msg);
    }
    #[test]
    fn it_decodes_attached() {
        let r = value_for_testfile("attached.plist");
        let msg = DeviceEventMessage::try_from(&r);
        assert!(msg.is_ok());
        let msg = msg.unwrap();
        assert!(msg.device_properties.is_some());
        assert_eq!(msg.message_type, MessageType::Attached);
        assert_eq!(msg.device_id, 3);
        let props = msg.device_properties.unwrap();
        assert_eq!(props.connection_type, "USB");
        assert_eq!(props.device_id, 3);
        assert_eq!(props.location_id, 0);
        assert_eq!(props.product_id, 4779);
        assert_eq!(props.serial_number, "00001011-000A111E0111001E");
    }

    #[test]
    fn it_decodes_command() {
        let command: Command = plist::from_file("test_data/command.plist").unwrap();
        assert_eq!(command.message_type, "Listen");
        assert_eq!(command.prog_name, "MyApp");
        assert_eq!(command.client_version_string, "1.0");
    }
}
