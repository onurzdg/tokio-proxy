use std::borrow::Cow;

pub trait AsDescription {
    fn as_description(&self) -> Cow<'static, str>;
}
