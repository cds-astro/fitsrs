use super::TableData;
use crate::error::Error;
use crate::hdu::header::extension::bintable::TFormType;
use crate::DataValue;

use serde::de::value::SeqDeserializer;
use serde::de::{DeserializeOwned, DeserializeSeed, Deserializer, MapAccess, Visitor};

use std::{
    cell::OnceCell,
    io::{Read, Seek},
};

use std::fmt::Debug;

pub struct RowDeserializeIter<R, T> {
    data: TableData<R>,
    ttypes: Vec<Option<String>>,
    tforms: Vec<TFormType>,
    row_idx: usize,
    num_rows: usize,
    sorted_fields: OnceCell<Vec<(&'static str, usize)>>,
    _marker: std::marker::PhantomData<T>,
}

impl<R, T> RowDeserializeIter<R, T> {
    pub(crate) fn new(
        data: TableData<R>,
        tforms: Vec<TFormType>,
        ttypes: Vec<Option<String>>,
        num_rows: usize,
    ) -> Self {
        Self {
            data,
            ttypes,
            tforms,
            num_rows,
            row_idx: 0,
            sorted_fields: OnceCell::new(),
            _marker: std::marker::PhantomData,
        }
    }
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
            &self.tforms,
            &self.sorted_fields,
        ));
        Some(result)
    }
}

pub struct RowDeserializer<'a, R, T> {
    data: &'a mut TableData<R>,
    ttypes: &'a [Option<String>], // TTYPEn names, same order/index as `row`
    tforms: &'a [TFormType],
    sorted_fields: &'a OnceCell<Vec<(&'static str, usize)>>,
    marker: std::marker::PhantomData<T>,
}

impl<'a, R, T> RowDeserializer<'a, R, T> {
    pub fn new(
        data: &'a mut TableData<R>,
        ttypes: &'a [Option<String>],
        tforms: &'a [TFormType],
        sorted_fields: &'a OnceCell<Vec<(&'static str, usize)>>,
    ) -> Self {
        Self {
            data,
            ttypes,
            tforms,
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
        let sorted_fields = if let Some(fields) = self.sorted_fields.get() {
            fields
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
            let mut sorted_fields = fields
                .iter()
                .cloned()
                .zip(fields.iter().map(|field| {
                    let ttype_idx = self
                        .ttypes
                        .iter()
                        .position(|c| {
                            c.as_ref()
                                .map(|c| c.eq_ignore_ascii_case(field))
                                .unwrap_or(false)
                        })
                        .unwrap();

                    ttype_idx
                }))
                .collect::<Vec<_>>();
            sorted_fields.sort_unstable_by_key(|(_, ttype_idx)| *ttype_idx);

            let selected_fields = sorted_fields
                .iter()
                .map(|(_, idx)| ColumnId::Index(*idx))
                .collect::<Vec<_>>();

            self.data.select_fields(&selected_fields);

            // safe: we just confirmed the cell was empty via `.get()` above
            self.sorted_fields.set(sorted_fields).ok();
            self.sorted_fields.get().unwrap()
        };

        visitor.visit_map(RowMapAccess {
            data: self.data,
            fields: sorted_fields,
            tforms: self.tforms,
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
    fields: &'a [(&'static str, usize)],
    tforms: &'a [TFormType],
    idx: usize,
}

impl<'a, R> RowMapAccess<'a, R>
where
    R: Read + Seek + Debug,
{
    fn next_values(&mut self, first: DataValue, count: usize) -> Result<Vec<DataValue>, Error> {
        let mut values = Vec::with_capacity(count);
        values.push(first);

        for _ in 1..count {
            values.push(
                self.data
                    .next()
                    .ok_or(Error::StaticError("No more values found."))?,
            );
        }

        Ok(values)
    }
}

use serde::de::IntoDeserializer;
impl<'de, R> MapAccess<'de> for RowMapAccess<'de, R>
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

        let field = self.fields[self.idx].0;
        seed.deserialize(field.into_deserializer()).map(Some)
    }

    fn next_value_seed<V>(&mut self, seed: V) -> Result<V::Value, Self::Error>
    where
        V: DeserializeSeed<'de>,
    {
        let field_idx = self.fields[self.idx].1;
        let tform = &self.tforms[field_idx];

        self.idx += 1;

        let value = self
            .data
            .next()
            .ok_or(Error::StaticError("No more values found."))?;

        match &value {
            DataValue::VariableLengthArray32 { num_elems, .. } => {
                let num_elems = *num_elems as usize;
                let next_values = self.next_values(value, num_elems)?;

                let deser = SeqDeserializer::new(next_values.into_iter());
                seed.deserialize(deser)
            }
            DataValue::VariableLengthArray64 { num_elems, .. } => {
                let num_elems = *num_elems as usize;
                let next_values = self.next_values(value, num_elems)?;

                let deser = SeqDeserializer::new(next_values.into_iter());
                seed.deserialize(deser)
            }
            _ => {
                let rc = tform.repeat_count();
                if rc > 1 {
                    let next_values = self.next_values(value, rc)?;

                    let deser = SeqDeserializer::new(next_values.into_iter());
                    seed.deserialize(deser)
                } else {
                    seed.deserialize(value)
                }
            }
        }
    }
}
