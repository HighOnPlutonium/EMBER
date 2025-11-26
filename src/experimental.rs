use std::cell::UnsafeCell;
use std::fmt;
use std::marker::PhantomData;
use std::mem::MaybeUninit;
use std::ops::Deref;
use std::panic::{RefUnwindSafe, UnwindSafe};
use std::sync::Once;

pub struct Antistatic<T> {
    once: Once,
    inner: UnsafeCell<MaybeUninit<T>>,
    _phantom_data: PhantomData<T> //IDK if necessary but they used PhantomData for OnceLock
}
impl<T> Antistatic<T> {
    pub const fn new() -> Antistatic<T> {
        Self {
            once: Once::new(),
            inner: UnsafeCell::new(MaybeUninit::uninit()),
            _phantom_data: PhantomData,
        }
    }
    //#[inline] //maybe?
    pub fn set(&self, value: T) {
        self.once.call_once_force(|p|{
            unsafe { (&mut *self.inner.get()).write(value); }
        });
    }
}
impl<T> Deref for Antistatic<T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        unsafe { (&*self.inner.get()).assume_init_ref() }
    }
}
//IDK if it's necessary but it might be?
impl<T> Drop for Antistatic<T> {
    #[inline]
    fn drop(&mut self) {
        if self.once.is_completed() {
            unsafe { (&mut *self.inner.get()).assume_init_drop() };
        }
    }
}

unsafe impl<T> Sync for Antistatic<T> {}
impl<T: RefUnwindSafe + UnwindSafe> RefUnwindSafe for Antistatic<T> {}
impl<T: UnwindSafe> UnwindSafe for Antistatic<T> {}

impl<T> Default for Antistatic<T> {
    #[inline]
    fn default() -> Antistatic<T> { Antistatic::new() }
}
impl<T: fmt::Debug> fmt::Debug for Antistatic<T> {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>)
        -> fmt::Result
    {
        let mut dbg = fmt.debug_tuple("Antistatic");
        if self.once.is_completed() { dbg.field(self.deref()); }
        else { dbg.field(&"<uninit>"); }
        dbg.finish()
    }
}
impl<T: Clone> Clone for Antistatic<T> {
    #[inline]
    fn clone(&self) -> Self {
        let cell = Self::new();
        cell.set(self.deref().clone());
        cell
    }
}