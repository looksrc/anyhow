// 用户解析`anyhow!($expr)`行为的标签分发机制.
// <br>
// Tagged dispatch mechanism for resolving the behavior of `anyhow!($expr)`.
//
// $expr有三种可能类型,分别实现以下三个特征,特征提供了方法anyhow_kind()返回三种单元值[Adhoc|Trait|Boxed]
// - AdhocKind 实现者: 实现了Debug+Display的类型 T: ?Sized + Display + Debug + Send + Sync + 'static {}
// - TraitKind 实现者: 实现了Into<anyhow::Error>类型,或anyhow::Error实现的From<E>类型
// - BoxedKind 实现者: 盒装的StdError对象, Box<dyn StdError + Send + Sync>
//
// 三种单元值都提供了new()方法创建anyhow::Error实例
// - Adhoc::new(...) 传入 T: ?Sized + Display + Debug + Send + Sync + 'static {}
// - Trait::new(...) 传入 E: Into<anyhow::Error>
// - Boxed::new(...) 传入 Box<dyn StdError + Send + Sync>
//
// 总结起来:
// - 1.创建anyhow::Error有三种可能的输入类型,分别为他们实现特征: AdhocKind,TraitKind,BoxedKind
// - 2.三种特征都提供了anyhow_kind()方法,以获取到三种对应的单元值: Adhoc,Trait,Boxed
// - 3.三个单元值提都供了从其对应的类型创建anyhow::Error的方法new(...)传入对应的类型值,创建出了anyhow::Error实例
//
// 三种单元值创建anyhow::Error实例时底层分别用了三种方法
// - 1.Adhoc::new: Error::from_adhoc(msg, backtrace)
// - 2.Trait::new: error.into()
// - 3.Boxed::new: Error::from_boxed(msg, backtrace)
//
// anyhow!()传入的表达式的值必须实现Debug和Display.
// 当传入表达式同时还实现了std::error::Error,则最终的anyhow::Error承接它的source()和backtrace().
// <br>
// When anyhow! is given a single expr argument to turn into anyhow::Error, we
// want the resulting Error to pick up the input's implementation of source()
// and backtrace() if it has a std::error::Error impl, otherwise require nothing
// more than Display and Debug.
//
// Expressed in terms of specialization, we want something like:
//
//     trait AnyhowNew {
//         fn new(self) -> Error;
//     }
//
//     impl<T> AnyhowNew for T
//     where
//         T: Display + Debug + Send + Sync + 'static,
//     {
//         default fn new(self) -> Error {
//             /* no std error impl */
//         }
//     }
//
//     impl<T> AnyhowNew for T
//     where
//         T: std::error::Error + Send + Sync + 'static,
//     {
//         fn new(self) -> Error {
//             /* use std error's source() and backtrace() */
//         }
//     }
//
// Since specialization is not stable yet, instead we rely on autoref behavior
// of method resolution to perform tagged dispatch. Here we have two traits
// AdhocKind and TraitKind that both have an anyhow_kind() method. AdhocKind is
// implemented whether or not the caller's type has a std error impl, while
// TraitKind is implemented only when a std error impl does exist. The ambiguity
// is resolved by AdhocKind requiring an extra autoref so that it has lower
// precedence.
//
// The anyhow! macro will set up the call in this form:
//
//     #[allow(unused_imports)]
//     use $crate::__private::{AdhocKind, TraitKind};
//     let error = $msg;
//     (&error).anyhow_kind().new(error)

use crate::Error;
use core::fmt::{Debug, Display};

#[cfg(feature = "std")]
use crate::StdError;

/// AdhocKind 实现者: 实现了Debug+Display的类型 T: ?Sized + Display + Debug + Send + Sync + 'static {}
///
pub struct Adhoc;

#[doc(hidden)]
pub trait AdhocKind: Sized {
    #[inline]
    fn anyhow_kind(&self) -> Adhoc {
        Adhoc
    }
}

impl<T> AdhocKind for &T where T: ?Sized + Display + Debug + Send + Sync + 'static {}

impl Adhoc {
    #[cold]
    pub fn new<M>(self, message: M) -> Error
    where
        M: Display + Debug + Send + Sync + 'static,
    {
        Error::from_adhoc(message, backtrace!())
    }
}

/// TraitKind 实现者: 实现了Into<anyhow::Error>类型,或anyhow::Error实现的From<E>类型
pub struct Trait;

#[doc(hidden)]
pub trait TraitKind: Sized {
    #[inline]
    fn anyhow_kind(&self) -> Trait {
        Trait
    }
}

impl<E> TraitKind for E where E: Into<Error> {}

impl Trait {
    #[cold]
    pub fn new<E>(self, error: E) -> Error
    where
        E: Into<Error>,
    {
        error.into()
    }
}

/// BoxedKind 实现者: 盒装的StdError对象, Box<dyn StdError + Send + Sync>
#[cfg(feature = "std")]
pub struct Boxed;

#[cfg(feature = "std")]
#[doc(hidden)]
pub trait BoxedKind: Sized {
    #[inline]
    fn anyhow_kind(&self) -> Boxed {
        Boxed
    }
}

#[cfg(feature = "std")]
impl BoxedKind for Box<dyn StdError + Send + Sync> {}

#[cfg(feature = "std")]
impl Boxed {
    #[cold]
    pub fn new(self, error: Box<dyn StdError + Send + Sync>) -> Error {
        let backtrace = backtrace_if_absent!(&*error);
        Error::from_boxed(error, backtrace)
    }
}
