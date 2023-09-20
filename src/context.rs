use crate::error::ContextError;
use crate::{Context, Error, StdError};
use core::convert::Infallible;
use core::fmt::{self, Debug, Display, Write};

#[cfg(backtrace)]
use std::error::Request;

mod ext {
    use super::*;

    /// 特征提供扩展上下文的功能
    pub trait StdError {
        fn ext_context<C>(self, context: C) -> Error
        where
            C: Display + Send + Sync + 'static;
    }

    /// 实现StdError,为标准错误类型附加扩展上下文方法ext_context()
    #[cfg(feature = "std")]
    impl<E> StdError for E
    where
        E: std::error::Error + Send + Sync + 'static,
    {
        fn ext_context<C>(self, context: C) -> Error
        where
            C: Display + Send + Sync + 'static,
        {
            // 如果当前错误没有提供了backtrace则捕获当前位置backtrace
            let backtrace = backtrace_if_absent!(&self);
            Error::from_context(context, self, backtrace)
        }
    }

    /// 实现StdError,为anyhow::Error附加扩展上下文的方法ext_context()
    impl StdError for Error {
        fn ext_context<C>(self, context: C) -> Error
        where
            C: Display + Send + Sync + 'static,
        {
            self.context(context)
        }
    }
}

/// 为Result<T,E>实现Context附加context和context_with方法.
/// 这里的E必须满足实现过StdError,即被附加过ext_context方法.
///
/// E目前有两种具体类型可取:
/// - 标准错误类型
/// - anyhow::Error
///
/// 附加上下文后的结果:
/// - Ok: 直接返回
/// - Err(error): 通过ext_context(...)附加上下文后返回Err(anyhow::Error)
///
/// 提供的两个方法:
/// - context(self,C): 上下文的值是先计算好的
/// - context(self,F): 上下文的值延迟计算,只有在Err情况下才计算,以节省性能
///
impl<T, E> Context<T, E> for Result<T, E>
where
    E: ext::StdError + Send + Sync + 'static,
{
    fn context<C>(self, context: C) -> Result<T, Error>
    where
        C: Display + Send + Sync + 'static,
    {
        // Not using map_err to save 2 useless frames off the captured backtrace
        // in ext_context.
        match self {
            Ok(ok) => Ok(ok),
            Err(error) => Err(error.ext_context(context)),
        }
    }

    fn with_context<C, F>(self, context: F) -> Result<T, Error>
    where
        C: Display + Send + Sync + 'static,
        F: FnOnce() -> C,
    {
        match self {
            Ok(ok) => Ok(ok),
            Err(error) => Err(error.ext_context(context())),
        }
    }
}

/// 为Option<T>实现Context,附加上下文后会转为Result<T,E>
/// - Some(T): Ok(T)
/// - None: 通过Error::from_display用上下文+backtrace创建anyhow::Error
///
/// ## 例子
/// ```
/// # type T = ();
/// #
/// use anyhow::{Context, Result};
///
/// fn maybe_get() -> Option<T> {
///     # const IGNORE: &str = stringify! {
///     ...
///     # };
///     # unimplemented!()
/// }
///
/// fn demo() -> Result<()> {
///     let t = maybe_get().context("there is no T")?;
///     # const IGNORE: &str = stringify! {
///     ...
///     # };
///     # unimplemented!()
/// }
/// ```
impl<T> Context<T, Infallible> for Option<T> {
    fn context<C>(self, context: C) -> Result<T, Error>
    where
        C: Display + Send + Sync + 'static,
    {
        // Not using ok_or_else to save 2 useless frames off the captured
        // backtrace.
        match self {
            Some(ok) => Ok(ok),
            None => Err(Error::from_display(context, backtrace!())),
        }
    }

    fn with_context<C, F>(self, context: F) -> Result<T, Error>
    where
        C: Display + Send + Sync + 'static,
        F: FnOnce() -> C,
    {
        match self {
            Some(ok) => Ok(ok),
            None => Err(Error::from_display(context(), backtrace!())),
        }
    }
}

impl<C, E> Debug for ContextError<C, E>
where
    C: Display,
    E: Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Error")
            .field("context", &Quoted(&self.context))
            .field("source", &self.error)
            .finish()
    }
}

impl<C, E> Display for ContextError<C, E>
where
    C: Display,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        Display::fmt(&self.context, f)
    }
}

impl<C, E> StdError for ContextError<C, E>
where
    C: Display,
    E: StdError + 'static,
{
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        Some(&self.error)
    }

    #[cfg(backtrace)]
    fn provide<'a>(&'a self, request: &mut Request<'a>) {
        StdError::provide(&self.error, request);
    }
}

/// 为CnotextError实现标准错误接口std::error::Error
/// - source: 取内部error::Own<ImplError<()>这个错误并转为标准错误特征对象
/// - provide: 透传给内部error字段的provide
impl<C> StdError for ContextError<C, Error>
where
    C: Display,
{
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        Some(unsafe { crate::ErrorImpl::error(self.error.inner.by_ref()) })
    }

    #[cfg(backtrace)]
    fn provide<'a>(&'a self, request: &mut Request<'a>) {
        Error::provide(&self.error, request);
    }
}

/// 同时用于两个完全不相干的功能
/// - 对Formatter进行包装,然后实现Write特征,这样就可以在格式化的时候添加自定义的转义逻辑
/// - 对类型进行包装,使之实现Debug时打印出的形式为"xxxx"形式,其中xxxx为原字符串字面量形式的转义.
struct Quoted<C>(C);

impl<C> Debug for Quoted<C>
where
    C: Display,
{
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_char('"')?;
        Quoted(&mut *formatter).write_fmt(format_args!("{}", self.0))?;
        formatter.write_char('"')?;
        Ok(())
    }
}

impl Write for Quoted<&mut fmt::Formatter<'_>> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        Display::fmt(&s.escape_debug(), self.0)
    }
}

pub(crate) mod private {
    use super::*;

    pub trait Sealed {}

    impl<T, E> Sealed for Result<T, E> where E: ext::StdError {}
    impl<T> Sealed for Option<T> {}
}
