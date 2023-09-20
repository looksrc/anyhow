//! 自定义了三个透明包装类型
//! - Own<T>: 带所有权的智能指针,指向堆数据..模拟Box<T>
//! - Ref<'a, T>: 自定义共享引用,引用某堆上数据..模拟&T
//! - Mut<'a, T>: 自定义可变引用,引用某堆上数据..模拟&mut T
//! 这三个类型内部都包裹的是指向堆数据的裸指针: ptr: NonNull<T>

use alloc::boxed::Box;
use core::marker::PhantomData;
use core::ptr::NonNull;

/// 有内部数据所有权的指针.实现了Send,Sync,Copy,Clone.
#[repr(transparent)]
pub struct Own<T>
where
    T: ?Sized,
{
    pub ptr: NonNull<T>,
}

unsafe impl<T> Send for Own<T> where T: ?Sized {}

unsafe impl<T> Sync for Own<T> where T: ?Sized {}

impl<T> Copy for Own<T> where T: ?Sized {}

impl<T> Clone for Own<T>
where
    T: ?Sized,
{
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Own<T>
where
    T: ?Sized,
{
    /// 从Box创建
    pub fn new(ptr: Box<T>) -> Self {
        Own {
            ptr: unsafe { NonNull::new_unchecked(Box::into_raw(ptr)) },
        }
    }

    /// 将内部类型T转为内部类型U,本质是将*const T -> *const U
    pub fn cast<U: CastTo>(self) -> Own<U::Target> {
        Own {
            ptr: self.ptr.cast(),
        }
    }

    /// 将底层数据装盒
    pub unsafe fn boxed(self) -> Box<T> {
        Box::from_raw(self.ptr.as_ptr())
    }

    /// 获取自定义引用..不占据数据所有权
    pub fn by_ref(&self) -> Ref<T> {
        Ref {
            ptr: self.ptr,
            lifetime: PhantomData,
        }
    }

    /// 获取自定义可变引用..不占据数据所有权
    pub fn by_mut(&mut self) -> Mut<T> {
        Mut {
            ptr: self.ptr,
            lifetime: PhantomData,
        }
    }
}

/// 自定义共享引用,功能同&T类似,可以存在多个共享引用,不能修改和移除数据
#[repr(transparent)]
pub struct Ref<'a, T>
where
    T: ?Sized,
{
    pub ptr: NonNull<T>,
    lifetime: PhantomData<&'a T>,
}

impl<'a, T> Copy for Ref<'a, T> where T: ?Sized {}

impl<'a, T> Clone for Ref<'a, T>
where
    T: ?Sized,
{
    fn clone(&self) -> Self {
        *self
    }
}

impl<'a, T> Ref<'a, T>
where
    T: ?Sized,
{
    /// 从共享引用创建
    pub fn new(ptr: &'a T) -> Self {
        Ref {
            ptr: NonNull::from(ptr),
            lifetime: PhantomData,
        }
    }

    /// 从裸指针创建
    #[cfg(not(anyhow_no_ptr_addr_of))]
    pub fn from_raw(ptr: NonNull<T>) -> Self {
        Ref {
            ptr,
            lifetime: PhantomData,
        }
    }

    /// 将内包类型T的可变引用转为内包类型为U的可变引用,本质是转了一下内部裸指针类型
    pub fn cast<U: CastTo>(self) -> Ref<'a, U::Target> {
        Ref {
            ptr: self.ptr.cast(),
            lifetime: PhantomData,
        }
    }

    /// 转为可变引用
    #[cfg(not(anyhow_no_ptr_addr_of))]
    pub fn by_mut(self) -> Mut<'a, T> {
        Mut {
            ptr: self.ptr,
            lifetime: PhantomData,
        }
    }

    /// 解引用,获取到内部数据的裸指针
    #[cfg(not(anyhow_no_ptr_addr_of))]
    pub fn as_ptr(self) -> *const T {
        self.ptr.as_ptr() as *const T
    }

    /// 解引用,获取到内部数据的共享引用&T
    pub unsafe fn deref(self) -> &'a T {
        &*self.ptr.as_ptr()
    }
}

/// 自定义可变引用,功能与&mut T类似可以对内部指向的数据进行修改和移除
#[repr(transparent)]
pub struct Mut<'a, T>
where
    T: ?Sized,
{
    pub ptr: NonNull<T>,
    lifetime: PhantomData<&'a mut T>,
}

impl<'a, T> Copy for Mut<'a, T> where T: ?Sized {}

impl<'a, T> Clone for Mut<'a, T>
where
    T: ?Sized,
{
    fn clone(&self) -> Self {
        *self
    }
}

impl<'a, T> Mut<'a, T>
where
    T: ?Sized,
{
    #[cfg(anyhow_no_ptr_addr_of)]
    pub fn new(ptr: &'a mut T) -> Self {
        Mut {
            ptr: NonNull::from(ptr),
            lifetime: PhantomData,
        }
    }

    /// 将内包类型T的可变引用转为内包类型为U的可变引用,本质是转了一下内部裸指针类型
    pub fn cast<U: CastTo>(self) -> Mut<'a, U::Target> {
        Mut {
            ptr: self.ptr.cast(),
            lifetime: PhantomData,
        }
    }

    /// 原可变引用转为一个共享引用Ref<'a,T>
    #[cfg(not(anyhow_no_ptr_addr_of))]
    pub fn by_ref(self) -> Ref<'a, T> {
        Ref {
            ptr: self.ptr,
            lifetime: PhantomData,
        }
    }

    /// 原可变引用转为一个全新的Mut<'a,T>
    pub fn extend<'b>(self) -> Mut<'b, T> {
        Mut {
            ptr: self.ptr,
            lifetime: PhantomData,
        }
    }

    /// 解引用,获取到内部数据的可变引用&mut T
    pub unsafe fn deref_mut(self) -> &'a mut T {
        &mut *self.ptr.as_ptr()
    }
}

impl<'a, T> Mut<'a, T> {
    /// 读出内存中数据
    pub unsafe fn read(self) -> T {
        self.ptr.as_ptr().read()
    }
}

/// 强制所有.cast调用都必须指定泛型参数: .cast::<U>().
/// 由于所有类型U都实现了CastTo且Target=U[自身],因此.cast::<U>()本质是强转为U类型
/// Force turbofish on all calls of `.cast::<U>()`.
pub trait CastTo {
    type Target;
}

impl<T> CastTo for T {
    type Target = T;
}
