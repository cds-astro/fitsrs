extern crate fitsrs;
extern crate js_sys;
extern crate wasm_bindgen;
// see https://rustwasm.github.io/wasm-bindgen/
use wasm_bindgen::prelude::*;

use fitsrs::{Bitpix, DataType, FITSHeaderKeyword, FITSKeywordValue, Fits};
use js_sys::{Float32Array, Float64Array, Int16Array, Int32Array, Uint8Array};

#[wasm_bindgen(js_name = read)]
pub fn read(bytes: Vec<u8>) -> Result<js_sys::Object, JsValue> {
    let Fits { data, header } = Fits::from_byte_slice(&bytes).map_err(|e| {
        // Map the error returned by fitsrs to a JS compatible error
        JsValue::from(&e.to_string())
    })?;

    // Convert the data to a JS array
    // depending on the value
    let data: JsValue = match data {
        DataType::F32(v) => {
            let v: Float32Array = v.0.as_slice().into();
            v.into()
        }
        DataType::F64(v) => {
            let v: Float64Array = v.0.as_slice().into();
            v.into()
        }
        DataType::U8(v) => {
            let v: Uint8Array = v.0.into();
            v.into()
        }
        DataType::I16(v) => {
            let v: Int16Array = v.0.as_slice().into();
            v.into()
        }
        DataType::I32(v) => {
            let v: Int32Array = v.0.as_slice().into();
            v.into()
        }
        _ => return Err("format I64 not supported".into()),
    };

    let js_header_obj = js_sys::Object::new();
    for card in header.cards.into_iter() {
        let (key, value) = match card.1 {
            FITSHeaderKeyword::Simple => (String::from("SIMPLE"), JsValue::undefined()),
            FITSHeaderKeyword::Bitpix(b) => {
                let value = match b {
                    Bitpix::F32 => -32,
                    Bitpix::F64 => -64,
                    Bitpix::U8 => 8,
                    Bitpix::I16 => 16,
                    Bitpix::I32 => 32,
                    Bitpix::I64 => 64,
                };

                (String::from("BITPIX"), JsValue::from_f64(value as f64))
            }
            FITSHeaderKeyword::Naxis(naxis) => {
                (String::from("NAXIS"), JsValue::from_f64(naxis as f64))
            }
            FITSHeaderKeyword::NaxisSize {
                // Index of the axis
                idx,
                // Size of the axis
                size,
                ..
            } => {
                let key = format!("NAXIS{idx}");
                (key, JsValue::from_f64(size as f64))
            }
            FITSHeaderKeyword::Blank(value) => {
                (String::from("BLANK"), JsValue::from_f64(value as f64))
            }
            // TODO we will probably need a Cow<str> here
            // because we have to delete simple quote doublons
            FITSHeaderKeyword::Comment(s) => (String::from("COMMENT"), JsValue::from_str(s)),
            FITSHeaderKeyword::History(s) => (String::from("HISTORY"), JsValue::from_str(s)),
            FITSHeaderKeyword::Other { name, value } => {
                let value = match value {
                    FITSKeywordValue::IntegerNumber(v) => JsValue::from_f64(v as f64),
                    FITSKeywordValue::Logical(v) => JsValue::from_bool(v),
                    FITSKeywordValue::CharacterString(v) => JsValue::from_str(v),
                    FITSKeywordValue::FloatingPoint(v) => JsValue::from_f64(v),
                    FITSKeywordValue::Undefined => JsValue::undefined(),
                };
                let name = std::str::from_utf8(name).map_err(|e| {
                    // Map the error returned by fitsrs to a JS compatible error
                    JsValue::from(&e.to_string())
                })?;
                (String::from(name), value)
            }
            FITSHeaderKeyword::End => (String::from("END"), JsValue::undefined()),
        };
        js_sys::Reflect::set(&js_header_obj, &key.into(), &value)?;
    }

    let res = js_sys::Object::new();
    js_sys::Reflect::set(&res, &"header".into(), &js_header_obj)?;
    js_sys::Reflect::set(&res, &"data".into(), &data)?;

    Ok(res)
}
