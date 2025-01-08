use std::io::BufRead;

use super::Slice;

pub trait MemoryLayout<'a> {
    type Type;

    fn get_data(&'a self) -> Self::Type;
    //fn get_data_mut(&mut self) -> &mut Self::Type;
}
impl<'a> MemoryLayout<'a> for Slice<'a> {
    type Type = &'a Self;

    fn get_data(&'a self) -> Self::Type {
        self
    }
}
use super::iter::Iter;
impl<'a, R> MemoryLayout<'a> for Iter<'a, R>
where
    R: BufRead,
{
    type Type = &'a Self;

    fn get_data(&'a self) -> Self::Type {
        self
    }
}
