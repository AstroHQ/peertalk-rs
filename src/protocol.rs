// use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use plist::Value;
// use serde::{Deserialize, Serialize};
use std::convert::TryFrom;
use std::error::Error;
use std::fmt;
use std::io::{Read, Seek};

#[derive(Debug)]
pub enum ProtocolError {
    InvalidMessageType(String),
    InvalidPlistEntry,
    InvalidPlistEntryForKey(&'static str),
}
impl fmt::Display for ProtocolError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ProtocolError::InvalidMessageType(t) => write!(f, "Invalid message type: {}", t),
            ProtocolError::InvalidPlistEntry => write!(f, "Invalid plist format/entry"),
            ProtocolError::InvalidPlistEntryForKey(key) => {
                write!(f, "Invalid plist entry for key: {}", key)
            }
        }
    }
}
impl Error for ProtocolError {
    // fn source(&self) -> Option<&(dyn Error + 'static)> { }
}

/*
Attach:
<plist version="1.0">
<dict>
        <key>DeviceID</key>
        <integer>3</integer>
        <key>MessageType</key>
        <string>Attached</string>
        <key>Properties</key>
        <dict>
                <key>ConnectionType</key>
                <string>USB</string>
                <key>DeviceID</key>
                <integer>3</integer>
                <key>LocationID</key>
                <integer>0</integer>
                <key>ProductID</key>
                <integer>4779</integer>
                <key>SerialNumber</key>
                <string>00008027-000C486C0222002E</string>
        </dict>
</dict>
</plist>
Detach:
<plist version="1.0">
<dict>
        <key>DeviceID</key>
        <integer>1</integer>
        <key>MessageType</key>
        <string>Detached</string>
</dict>
</plist>
Paired:
<plist version="1.0">
<dict>
        <key>DeviceID</key>
        <integer>1</integer>
        <key>MessageType</key>
        <string>Paired</string>
</dict>
</plist>
Result:
<plist version="1.0">
<dict>
        <key>MessageType</key>
        <string>Result</string>
        <key>Number</key>
        <integer>0</integer>
</dict>
</plist>
*/
#[derive(Debug)]
pub enum MessageType {
    Paired,
    Result,
    Detached,
    Attached,
}
impl TryFrom<&Value> for MessageType {
    type Error = ProtocolError;
    fn try_from(value: &Value) -> Result<Self, Self::Error> {
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
    fn try_from(value: &Value) -> Result<Self, Self::Error> {
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
    pub fn from_reader<R: Read + Seek>(reader: R) -> Result<Self, ProtocolError> {
        let r: plist::Value = plist::Value::from_reader(reader).unwrap();
        ResultMessage::try_from(&r)
    }
}
impl TryFrom<&Value> for ResultMessage {
    type Error = ProtocolError;
    fn try_from(value: &Value) -> Result<Self, Self::Error> {
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
    pub fn from_reader<R: Read + Seek>(reader: R) -> Result<Self, ProtocolError> {
        let r: plist::Value = plist::Value::from_reader(reader).unwrap();
        DeviceEventMessage::try_from(&r)
    }
}
impl TryFrom<&Value> for DeviceEventMessage {
    type Error = ProtocolError;
    fn try_from(value: &Value) -> Result<Self, Self::Error> {
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

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn it_works() {
        let r: plist::Value = plist::Value::from_file("test.plist").unwrap();
        let msg = DeviceEventMessage::try_from(&r).unwrap();
        println!("Test: {:?}", msg);
    }
}
