use std::marker::PhantomData;
use std::cell::UnsafeCell;

pub struct Opaque(PhantomData<UnsafeCell<*mut ()>>);

macro_rules! generic_foreign_type_and_impl_send_sync {
    (
        $(#[$impl_attr:meta])*
        type CType = $ctype:ty;
        fn drop = $drop:expr;
        $(fn clone = $clone:expr;)*

        $(#[$owned_attr:meta])*
        pub struct $owned:ident<T>;
        $(#[$borrowed_attr:meta])*
        pub struct $borrowed:ident<T>;
    ) => {
        $(#[$owned_attr])*
        pub struct $owned<T>(*mut $ctype, Box<T>);

        $(#[$borrowed_attr])*
        pub struct $borrowed<T>(Opaque, Box<T>);

        unsafe impl<T> Send for $owned<T>{}
        unsafe impl<T> Send for $borrowed<T>{}
        unsafe impl<T> Sync for $owned<T>{}
        unsafe impl<T> Sync for $borrowed<T>{}
    };
}

pub enum X509_LOOKUP_METHOD {}

extern "C" {
    pub fn X509_LOOKUP_meth_free(method: *mut X509_LOOKUP_METHOD);
}

generic_foreign_type_and_impl_send_sync! {
    type CType = X509_LOOKUP_METHOD;
    fn drop = |_method| {
        ffi::X509_LOOKUP_meth_free(_method);
    };

    /// Method used to look up certificates and CRLs.
    pub struct X509LookupMethod<T>;
    /// Reference to an `X509LookupMethod`.
    pub struct X509LookupMethodRef<T>;
}
