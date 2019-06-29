use crate::types::{String, Value, ValueType};
use std::convert::TryFrom;
use std::ptr::null_mut;

use javascriptcore_sys::{
    JSStringGetMaximumUTF8CStringSize, JSStringGetUTF8CString, JSValueToNumber, JSValueToStringCopy,
};

impl TryFrom<&Value> for std::string::String {
    type Error = Box<dyn std::error::Error>;

    fn try_from(value: &Value) -> Result<std::string::String, Self::Error> {
        let js_string = String::try_from(value)?;
        Ok(std::string::String::from(&js_string))
    }
}

impl TryFrom<&Value> for String {
    type Error = Box<dyn std::error::Error>;

    fn try_from(value: &Value) -> Result<String, Self::Error> {
        match value.js_type() {
            ValueType::String => unsafe {
                let string = JSValueToStringCopy(*value.2, value.0, null_mut());
                if string == null_mut() {
                    panic!()
                }
                Ok(String(string))
            },
            _ => unimplemented!(),
        }
    }
}

impl TryFrom<&Value> for f64 {
    type Error = Box<dyn std::error::Error>;

    fn try_from(value: &Value) -> Result<f64, Self::Error> {
        match value.js_type() {
            ValueType::Number => Ok(unsafe { JSValueToNumber(*value.2, value.0, null_mut()) }),
            _ => unimplemented!(),
        }
    }
}

impl From<&String> for std::string::String {
    fn from(string: &String) -> std::string::String {
        let size = unsafe { JSStringGetMaximumUTF8CStringSize(string.0) };
        let mut buffer = vec![0u8; size];
        let written = unsafe { JSStringGetUTF8CString(string.0, buffer.as_mut_ptr() as _, size) };
        buffer.truncate(written);

        let c_str = unsafe { std::ffi::CStr::from_bytes_with_nul_unchecked(&*buffer) };
        c_str.to_str().unwrap().to_string()
    }
}
