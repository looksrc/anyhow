use crate::chain::Chain;
use crate::error::ErrorImpl;
use crate::ptr::Ref;
use core::fmt::{self, Debug, Write};

impl ErrorImpl {
    /// 打印错误
    pub(crate) unsafe fn display(this: Ref<Self>, f: &mut fmt::Formatter) -> fmt::Result {
        // 打印内部_object所化的标准错误特征对象
        write!(f, "{}", Self::error(this))?;

        // 如果格式化字符串中使用了#符号则打印错误链中的每个错误..比如{:#?}
        if f.alternate() {
            for cause in Self::chain(this).skip(1) {
                write!(f, ": {}", cause)?;
            }
        }

        Ok(())
    }

    /// 打印错误调试信息
    pub(crate) unsafe fn debug(this: Ref<Self>, f: &mut fmt::Formatter) -> fmt::Result {
        // 获取内部_object所化的标准错误特征对象
        let error = Self::error(this);

        // 如果格式化字符串使用了#符号,则直接对内部错误进行调试打印..入{:#}
        if f.alternate() {
            return Debug::fmt(error, f);
        }

        // 打印内部错误
        write!(f, "{}", error)?;

        // 如果存在多级错误,则打印错误链
        if let Some(cause) = error.source() {
            // 1.空一行打印
            write!(f, "\n\nCaused by:")?;
            // 2.查看是否有是多级错误链
            let multiple = cause.source().is_some();
            // 3.迭代遍历多级错误链,每级错误都带有一个层次编号,使用Indented进行缩进打印
            for (n, error) in Chain::new(cause).enumerate() {
                writeln!(f)?;
                let mut indented = Indented {
                    inner: f,
                    number: if multiple { Some(n) } else { None },
                    started: false,
                };
                write!(indented, "{}", error)?;
            }
        }

        // 如果开启了backtrace则打印跟踪信息
        #[cfg(any(backtrace, feature = "backtrace"))]
        {
            use crate::backtrace::BacktraceStatus;

            // 获取当前ErrorImpl实例的backtrace
            let backtrace = Self::backtrace(this);

            // 如果backtrace为已捕获状态,则开始打印
            if let BacktraceStatus::Captured = backtrace.status() {
                // 1.先将backtrace转为字符串
                let mut backtrace = backtrace.to_string();

                // 2.开始空一行
                write!(f, "\n\n")?;

                // 3.处理头部:
                // - 如果有"stack backtrace:"开头,则stack首字母大写
                // - 否则,直接写入"Stack backtrace:"
                if backtrace.starts_with("stack backtrace:") {
                    // Capitalize to match "Caused by:"
                    backtrace.replace_range(0..1, "S");
                } else {
                    // "stack backtrace:" prefix was removed in
                    // https://github.com/rust-lang/backtrace-rs/pull/286
                    writeln!(f, "Stack backtrace:")?;
                }

                // 4.移除尾部空白
                backtrace.truncate(backtrace.trim_end().len());

                // 5.写入backtrace字符串
                write!(f, "{}", backtrace)?;
            }
        }

        Ok(())
    }
}

/// 创建一个新的写入器包装原有的写入器,以添加缩进逻辑
///
/// 添加的自定义逻辑:
/// 1.有数字时,数字在首行输出占5个宽度右对齐后跟首行..后续行缩进7个空格
/// 2.没有数字时,首行和后续行都缩进4个空格
struct Indented<'a, D> {
    inner: &'a mut D,      // 内部写入器
    number: Option<usize>, // 首行携带的数字,如果不为None则数字会被宽度为5右对齐打印,后续行缩进7个空格,如果为None后续行缩进4个空格
    started: bool,         // 非首行标记,false表示首行
}

impl<T> Write for Indented<'_, T>
where
    T: Write,
{
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for (i, line) in s.split('\n').enumerate() {
            if !self.started {
                self.started = true;
                match self.number {
                    Some(number) => write!(self.inner, "{: >5}: ", number)?,
                    None => self.inner.write_str("    ")?,
                }
            } else if i > 0 {
                // 换行:
                // 如果不是第0行且不是起始行,则先写入换行
                self.inner.write_char('\n')?;

                // 缩进:
                // 如果number有值则写入7个空格
                // 如果number无值则写入4个空格
                if self.number.is_some() {
                    self.inner.write_str("       ")?;
                } else {
                    self.inner.write_str("    ")?;
                }
            }

            // 写入实际的行内容
            self.inner.write_str(line)?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 测试number是1位数字
    #[test]
    fn one_digit() {
        let input = "verify\nthis";
        let expected = "    2: verify\n       this";
        let mut output = String::new();

        Indented {
            inner: &mut output,
            number: Some(2),
            started: false,
        }
        .write_str(input)
        .unwrap();

        assert_eq!(expected, output);
    }

    // 测试number是2位数字
    #[test]
    fn two_digits() {
        let input = "verify\nthis";
        let expected = "   12: verify\n       this";
        let mut output = String::new();

        Indented {
            inner: &mut output,
            number: Some(12),
            started: false,
        }
        .write_str(input)
        .unwrap();

        assert_eq!(expected, output);
    }

    // 测试number不带数字
    #[test]
    fn no_digits() {
        let input = "verify\nthis";
        let expected = "    verify\n    this";
        let mut output = String::new();

        Indented {
            inner: &mut output,
            number: None,
            started: false,
        }
        .write_str(input)
        .unwrap();

        assert_eq!(expected, output);
    }
}
