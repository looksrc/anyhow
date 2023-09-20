//! ## 模块说明
//!

use self::ChainState::*;
use crate::StdError;

/// 如果启用std特性则导入vec模块
#[cfg(feature = "std")]
use std::vec;

/// 如果启用std特性则导入包级别Chain类型
#[cfg(feature = "std")]
pub(crate) use crate::Chain;

/// 如果不启用std则自定义Chain类型,与上一条互斥..(事实上两个Chain代码是相同的)
#[cfg(not(feature = "std"))]
pub(crate) struct Chain<'a> {
    state: ChainState<'a>,
}

/// 错误连Chain的两种状态
/// - Linked: 正常的错误链组织形式,只记录下一个错误对象..迭代时通过source()再获取下下一个
/// - Buffered: 向量迭代器形式,当需要进行双端迭代时,需要缓冲整个错误链中的所有对象到向量中,这样就可以通过next_back透传给向量迭代器来实现从后端迭代Chain
#[derive(Clone)]
pub(crate) enum ChainState<'a> {
    Linked {
        next: Option<&'a (dyn StdError + 'static)>,
    },
    #[cfg(feature = "std")]
    Buffered {
        rest: vec::IntoIter<&'a (dyn StdError + 'static)>,
    },
}

impl<'a> Chain<'a> {
    /// 创建并初始化错误链,实际上是将首个错误对象的引用作为next来创建ChainState::Linked
    #[cold]
    pub fn new(head: &'a (dyn StdError + 'static)) -> Self {
        Chain {
            state: ChainState::Linked { next: Some(head) },
        }
    }
}

/// 为错误链实现迭代器Iterator,迭代项类型为错误对象的引用
/// - 链接形式: 当前值为迭代值,处理完后通过source()获取下一个值并更新next字段
/// - 向量形式: 直接将迭代操作next()方法透传给向量迭代器
impl<'a> Iterator for Chain<'a> {
    type Item = &'a (dyn StdError + 'static);

    fn next(&mut self) -> Option<Self::Item> {
        match &mut self.state {
            Linked { next } => {
                let error = (*next)?;
                *next = error.source();
                Some(error)
            }
            #[cfg(feature = "std")]
            Buffered { rest } => rest.next(),
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = self.len();
        (len, Some(len))
    }
}

/// 实现双端迭代,即可以从任意一端向另一端迭代.
/// Iterator是从前向后迭代,DoubleEndedIterator只需要实现从后向前迭代就行了.
/// 
/// 实现逻辑:
/// - 迭代第一项时,如果当前ChainState为Linked状态则循环找出整个错误链缓存到向量Vec中,并更新Chain的状态为Buffered缓冲状态
/// - Chain变为缓冲状态后,通过将next_back()反向迭代头传给Buffered向量进行处理.
#[cfg(feature = "std")]
impl DoubleEndedIterator for Chain<'_> {
    fn next_back(&mut self) -> Option<Self::Item> {
        match &mut self.state {
            Linked { mut next } => {
                let mut rest = Vec::new();
                while let Some(cause) = next {
                    next = cause.source();
                    rest.push(cause);
                }
                let mut rest = rest.into_iter();
                let last = rest.next_back();
                self.state = Buffered { rest };
                last
            }
            Buffered { rest } => rest.next_back(),
        }
    }
}

/// 实现精确长度迭代,即实现len()方法取迭代目标的长度
impl ExactSizeIterator for Chain<'_> {
    fn len(&self) -> usize {
        match &self.state {
            Linked { mut next } => {
                let mut len = 0;
                while let Some(cause) = next {
                    next = cause.source();
                    len += 1;
                }
                len
            }
            #[cfg(feature = "std")]
            Buffered { rest } => rest.len(),
        }
    }
}

/// 开启std特性时,Chain的默认值状态为缓冲状态
#[cfg(feature = "std")]
impl Default for Chain<'_> {
    fn default() -> Self {
        Chain {
            state: ChainState::Buffered {
                rest: Vec::new().into_iter(),
            },
        }
    }
}
