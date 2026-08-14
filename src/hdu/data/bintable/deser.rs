use super::TableData;
use crate::error::Error;

use serde::de::{DeserializeOwned, DeserializeSeed, Deserializer, MapAccess, Visitor};

use std::{
    cell::OnceCell,
    io::{Read, Seek},
};

use std::fmt::Debug;

pub struct RowDeserializeIter<R, T> {
    pub data: TableData<R>,
    pub ttypes: Vec<Option<String>>,
    pub row_idx: usize,
    pub num_rows: usize,
    pub sorted_fields: OnceCell<Vec<&'static str>>,
    pub _marker: std::marker::PhantomData<T>,
}

impl<R, T> Iterator for RowDeserializeIter<R, T>
where
    R: Read + Seek + Debug,
    T: DeserializeOwned,
{
    type Item = Result<T, Error>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.row_idx >= self.num_rows {
            return None;
        }
        self.row_idx += 1;

        let result = T::deserialize(RowDeserializer::<R, T>::new(
            &mut self.data,
            &self.ttypes,
            &self.sorted_fields,
        ));
        Some(result)
    }
}

pub struct RowDeserializer<'a, R, T> {
    data: &'a mut TableData<R>,
    ttypes: &'a [Option<String>], // TTYPEn names, same order/index as `row`
    sorted_fields: &'a OnceCell<Vec<&'static str>>,
    marker: std::marker::PhantomData<T>,
}

impl<'a, R, T> RowDeserializer<'a, R, T> {
    pub fn new(
        data: &'a mut TableData<R>,
        ttypes: &'a [Option<String>],
        sorted_fields: &'a OnceCell<Vec<&'static str>>,
    ) -> Self {
        Self {
            data,
            ttypes,
            sorted_fields,
            marker: std::marker::PhantomData,
        }
    }
}

use crate::hdu::data::bintable::ColumnId;
impl<'de, T, R> Deserializer<'de> for RowDeserializer<'de, R, T>
where
    R: Debug + Read + Seek,
{
    type Error = Error;

    fn deserialize_struct<V>(
        self,
        _name: &'static str,
        fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        let sorted_fields = if let Some(sorted) = self.sorted_fields.get() {
            sorted
        } else {
            // validate first — bail out with a real error if a field is not found among the TTYPEs of the HDU
            for &field in fields {
                let ttype_idx = self.ttypes.iter().position(|c| {
                    c.as_ref()
                        .map(|c| c.eq_ignore_ascii_case(field))
                        .unwrap_or(false)
                });
                if ttype_idx.is_none() {
                    return Err(Error::DynamicError(format!(
                        "field '{field}' does not match any FITS column (TTYPE)"
                    )));
                }
            }

            // all fields validated, so the .unwrap()s below are now safe
            let mut sorted = fields.to_vec();
            sorted.sort_unstable_by_key(|field| {
                self.ttypes
                    .iter()
                    .position(|c| {
                        c.as_ref()
                            .map(|c| c.eq_ignore_ascii_case(field))
                            .unwrap_or(false)
                    })
                    .unwrap()
            });

            let selected_fields = sorted
                .iter()
                .map(|field| {
                    ColumnId::Index(
                        self.ttypes
                            .iter()
                            .position(|c| {
                                c.as_ref()
                                    .map(|c| c.eq_ignore_ascii_case(field))
                                    .unwrap_or(false)
                            })
                            .unwrap(),
                    )
                })
                .collect::<Vec<_>>();

            self.data.select_fields(&selected_fields);

            // safe: we just confirmed the cell was empty via `.get()` above
            self.sorted_fields.set(sorted).ok();
            self.sorted_fields.get().unwrap()
        };

        visitor.visit_map(RowMapAccess {
            data: self.data,
            fields: sorted_fields,
            idx: 0,
        })
    }

    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        // structs always go through deserialize_struct in practice via derive,
        // but keep this for completeness/robustness
        self.deserialize_struct("", &[], visitor)
    }

    serde::forward_to_deserialize_any! {
        bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string
        bytes byte_buf option unit unit_struct newtype_struct seq tuple
        tuple_struct map enum identifier ignored_any
    }
}

struct RowMapAccess<'a, R> {
    data: &'a mut TableData<R>,
    fields: &'a [&'static str],
    idx: usize,
}

use serde::de::IntoDeserializer;
impl<'de, 'a, R> MapAccess<'de> for RowMapAccess<'a, R>
where
    R: Read + Seek + Debug,
{
    type Error = Error;

    fn next_key_seed<K>(&mut self, seed: K) -> Result<Option<K::Value>, Self::Error>
    where
        K: DeserializeSeed<'de>,
    {
        if self.idx >= self.fields.len() {
            return Ok(None);
        }

        let field_name = self.fields[self.idx];
        seed.deserialize(field_name.into_deserializer()).map(Some)
    }

    fn next_value_seed<V>(&mut self, seed: V) -> Result<V::Value, Self::Error>
    where
        V: DeserializeSeed<'de>,
    {
        self.idx += 1;

        let value = self
            .data
            .next()
            .ok_or(Error::StaticError("No more values found."))?;
        seed.deserialize(value)
    }
}
