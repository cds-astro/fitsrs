extern crate fitsrs;
extern crate wasm_bindgen;
extern crate js_sys;
// see https://rustwasm.github.io/wasm-bindgen/
use wasm_bindgen::prelude::*;

use fitsrs::{Fits, DataType};
use js_sys::{Uint8Array, Int16Array, Int32Array, Float32Array, Float64Array};

#[wasm_bindgen(js_name = readPrimaryHDUData)]
pub fn read_data(bytes: Vec<u8>) -> Result<JsValue, JsValue> {
    let Fits { header: _header, data } = Fits::from_byte_slice(&bytes)
        .map_err(|e| {
            // Map the error returned by fitsrs to a Js error
            JsValue::from(&e.to_string())
        })?;

    let data: JsValue = match data {
        DataType::F32(v) => {
            let v: Float32Array = v.0.as_slice().into();
            v.into()
        },
        DataType::F64(v) => {
            let v: Float64Array = v.0.as_slice().into();
            v.into()
        },
        DataType::U8(v) => {
            let v: Uint8Array = v.0.into();
            v.into()
        },
        DataType::I16(v) => {
            let v: Int16Array = v.0.as_slice().into();
            v.into()
        },
        DataType::I32(v) => {
            let v: Int32Array = v.0.as_slice().into();
            v.into()
        },
        _ => {
            return Err("format I64 not supported".into())
        }
    };

    Ok(data)
}

