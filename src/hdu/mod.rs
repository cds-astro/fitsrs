mod header;
mod data;
pub use data::DataTypeBorrowed;
pub use header::Header;

struct HDU<'a> {
    header: Header,
    data: Data<'a>,
}