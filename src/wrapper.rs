//! 将三种目标类型包装为实现了std::error::Error的三种包装类型
//! - 1.M: Display + Debug --> MessageError<M>(M)
//! - 2.M: Display         --> DisplayError<M>(M)
//! - 3.Box<dyn StdError + Send + Sync> --> BoxedError(...)

use crate::StdError;
use core::fmt::{self, Debug, Display};

#[cfg(backtrace)]
use std::error::Request;

/// 将目标类型类型包装为实现了StdError的类型MessageError, 目标类型: "Display+Debug"
/// - Debug和Display分别透传给内部错误的Debug和Display
#[repr(transparent)]
pub struct MessageError<M>(pub M);

impl<M> Debug for MessageError<M>
where
    M: Display + Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        Debug::fmt(&self.0, f)
    }
}

impl<M> Display for MessageError<M>
where
    M: Display + Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        Display::fmt(&self.0, f)
    }
}

impl<M> StdError for MessageError<M> where M: Display + Debug + 'static {}

/// 将目标类型类型包装为实现了StdError的类型MessageError, 目标类型: "Display"
/// - Debug和Display都透传给内部错误的Display
#[repr(transparent)]
pub struct DisplayError<M>(pub M);

impl<M> Debug for DisplayError<M>
where
    M: Display,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        Display::fmt(&self.0, f)
    }
}

impl<M> Display for DisplayError<M>
where
    M: Display,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        Display::fmt(&self.0, f)
    }
}

impl<M> StdError for DisplayError<M> where M: Display + 'static {}

/// 将目标类型类型包装为实现了StdError的类型MessageError, 目标类型: "Box<dyn StdError + Send + Sync>"
/// 实现StdError的逻辑:
/// - 先实现Debug和Display特征,直接透传给内部对象的Debug和Display
/// - 再实现StdError的source和provide方法,也是直接透传给内部对象的source和.
#[cfg(feature = "std")]
#[repr(transparent)]
pub struct BoxedError(pub Box<dyn StdError + Send + Sync>);

/// 实现Debug: 调用内部内容的Debug
#[cfg(feature = "std")]
impl Debug for BoxedError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        Debug::fmt(&self.0, f)
    }
}

/// 实现Display: 调用内部内容的Display
#[cfg(feature = "std")]
impl Display for BoxedError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        Display::fmt(&self.0, f)
    }
}

/// 实现std::error::Error特征,透传source和provide给内部错误对象
#[cfg(feature = "std")]
impl StdError for BoxedError {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        self.0.source()
    }

    #[cfg(backtrace)]
    fn provide<'a>(&'a self, request: &mut Request<'a>) {
        self.0.provide(request);
    }
}
