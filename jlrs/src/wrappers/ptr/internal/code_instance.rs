//! Wrapper for `CodeInstance`.
//!
//! The documentation for this module has been slightly adapted from the comments for this struct
//! in [`julia.h`]
//!
//! [`julia.h`]: https://github.com/JuliaLang/julia/blob/96786e22ccabfdafd073122abb1fb69cea921e17/src/julia.h#L273

use std::{ffi::c_void, marker::PhantomData, ptr::NonNull};

use cfg_if::cfg_if;
use jl_sys::{jl_code_instance_t, jl_code_instance_type};

use crate::{
    impl_julia_typecheck,
    memory::target::Target,
    private::Private,
    wrappers::ptr::{
        private::WrapperPriv,
        value::{Value, ValueData, ValueRef},
        Ref,
    },
};

cfg_if! {
    if #[cfg(not(feature = "lts"))] {
        use std::{sync::atomic::Ordering, ptr::null_mut};
    }
}

/// A `CodeInstance` represents an executable operation.
#[derive(Copy, Clone)]
#[repr(transparent)]
pub struct CodeInstance<'scope>(NonNull<jl_code_instance_t>, PhantomData<&'scope ()>);

impl<'scope> CodeInstance<'scope> {
    /*
    inspect(Core.CodeInstance):

    def: Core.MethodInstance (const)
    next: Core.CodeInstance (mut)
    min_world: UInt64 (const)
    max_world: UInt64 (const)
    rettype: Any (const)
    rettype_const: Any (const)
    inferred: Any (mut)
    ipo_purity_bits: UInt32 (const)
    purity_bits: UInt32 (mut)
    argescapes: Any (const)
    isspecsig: Bool (mut)
    precompile: Bool (mut) _Atomic
    relocatability: UInt8 (mut)
    invoke: Ptr{Nothing} (mut) _Atomic
    specptr: Ptr{Nothing} (mut) _Atomic
    */

    /// Method this instance is specialized from.
    pub fn def(self) -> Option<MethodInstance<'scope>> {
        // Safety: the pointer points to valid data
        unsafe {
            let def = self.unwrap_non_null(Private).as_ref().def;
            Some(MethodInstance::wrap_non_null(NonNull::new(def)?, Private))
        }
    }

    /// Next cache entry.
    pub fn next<'target, T>(self, target: T) -> Option<CodeInstanceData<'target, T>>
    where
        T: Target<'target>,
    {
        cfg_if! {
            if #[cfg(feature = "lts")] {
                // Safety: the pointer points to valid data
                unsafe {
                    let next = self.unwrap_non_null(Private).as_ref().next;
                    Some(CodeInstanceRef::wrap(NonNull::new(next)?).root(target))
                }
            } else {
                // Safety: the pointer points to valid data
                unsafe {
                    let next = self.unwrap_non_null(Private).as_ref().next.load(Ordering::Relaxed);
                    Some(CodeInstanceRef::wrap(NonNull::new(next)?).root(target))
                }
            }
        }
    }

    /// Returns the minimum of the world range for which this object is valid to use.
    pub fn min_world(self) -> usize {
        // Safety: the pointer points to valid data
        unsafe { self.unwrap_non_null(Private).as_ref().min_world }
    }

    /// Returns the maximum of the world range for which this object is valid to use.
    pub fn max_world(self) -> usize {
        // Safety: the pointer points to valid data
        unsafe { self.unwrap_non_null(Private).as_ref().max_world }
    }

    /// Return type for fptr.
    pub fn rettype(self) -> Option<Value<'scope, 'static>> {
        // Safety: the pointer points to valid data
        unsafe {
            let rettype = self.unwrap_non_null(Private).as_ref().rettype;
            Some(Value::wrap_non_null(NonNull::new(rettype)?, Private))
        }
    }

    /// Inferred constant return value, or null
    pub fn rettype_const(self) -> Option<Value<'scope, 'static>> {
        // Safety: the pointer points to valid data
        unsafe {
            let rettype_const = self.unwrap_non_null(Private).as_ref().rettype_const;
            Some(Value::wrap_non_null(NonNull::new(rettype_const)?, Private))
        }
    }

    /// Inferred `CodeInfo`, `Nothing`, or `None`.
    pub fn inferred<'target, T>(self, target: T) -> Option<ValueData<'target, 'static, T>>
    where
        T: Target<'target>,
    {
        // Safety: the pointer points to valid data
        cfg_if! {
            if #[cfg(not(any(feature = "nightly", feature = "beta")))] {
                unsafe {
                    let inferred = self.unwrap_non_null(Private).as_ref().inferred;
                    Some(ValueRef::wrap(NonNull::new(inferred)?).root(target))
                }
            } else {
                // Safety: the pointer points to valid data
                unsafe {
                    let inferred = self.unwrap_non_null(Private).as_ref().inferred.load(Ordering::Relaxed);
                    Some(ValueRef::wrap(NonNull::new(inferred)?).root(target))
                }
            }
        }
    }

    /// The `ipo_purity_bits` field of this `CodeInstance`.
    #[cfg(not(feature = "lts"))]
    pub fn ipo_purity_bits(self) -> u32 {
        // Safety: the pointer points to valid data
        unsafe { self.unwrap_non_null(Private).as_ref().ipo_purity_bits }
    }

    /// The `purity_bits` field of this `CodeInstance`.
    #[cfg(not(feature = "lts"))]
    pub fn purity_bits(self) -> u32 {
        // Safety: the pointer points to valid data
        #[cfg(any(feature = "nightly", feature = "beta"))]
        unsafe {
            self.unwrap_non_null(Private)
                .as_ref()
                .purity_bits
                .load(Ordering::Relaxed)
        }
        #[cfg(not(any(feature = "nightly", feature = "beta")))]
        unsafe {
            self.unwrap_non_null(Private).as_ref().purity_bits
        }
    }

    /// Method this instance is specialized from.
    #[cfg(not(feature = "lts"))]
    pub fn argescapes(self) -> Option<Value<'scope, 'static>> {
        // Safety: the pointer points to valid data
        unsafe {
            let argescapes = self.unwrap_non_null(Private).as_ref().argescapes;
            Some(Value::wrap_non_null(NonNull::new(argescapes)?, Private))
        }
    }

    /// If `specptr` is a specialized function signature for specTypes->rettype
    pub fn is_specsig(self) -> bool {
        // Safety: the pointer points to valid data
        unsafe { self.unwrap_non_null(Private).as_ref().isspecsig != 0 }
    }

    /// If `specptr` is a specialized function signature for specTypes->rettype
    pub fn precompile(self) -> bool {
        cfg_if! {
            if #[cfg(feature = "lts")] {
                // Safety: the pointer points to valid data
                unsafe { self.unwrap_non_null(Private).as_ref().precompile != 0 }
            } else {
                // Safety: the pointer points to valid data
                unsafe {
                    self.unwrap_non_null(Private).as_ref().precompile.load(Ordering::Relaxed) != 0
                }
            }
        }
    }

    /// jlcall entry point
    pub fn invoke(self) -> *mut c_void {
        cfg_if! {
            if #[cfg(feature = "lts")] {
                use std::ptr::null_mut;
                // Safety: the pointer points to valid data
                unsafe { self.unwrap_non_null(Private).as_ref().invoke.map(|x| x as *mut c_void).unwrap_or(null_mut()) }
            } else {
                // Safety: the pointer points to valid data
                unsafe {
                    self.unwrap_non_null(Private).as_ref().invoke.load(Ordering::Relaxed).map(|x| x as *mut c_void).unwrap_or(null_mut())
                }
            }
        }
    }

    /// private data for `jlcall entry point
    pub fn specptr(self) -> *mut c_void {
        cfg_if! {
            if #[cfg(feature = "lts")] {
                // Safety: the pointer points to valid data
                unsafe { self.unwrap_non_null(Private).as_ref().specptr.fptr }
            } else {
                // Safety: the pointer points to valid data
                unsafe {
                    self.unwrap_non_null(Private).as_ref().specptr.fptr.load(Ordering::Relaxed)
                }
            }
        }
    }

    /// nonzero if all roots are built into sysimg or tagged by module key
    #[cfg(not(feature = "lts"))]
    pub fn relocatability(self) -> u8 {
        // Safety: the pointer points to valid data
        unsafe { self.unwrap_non_null(Private).as_ref().relocatability }
    }
}

impl_julia_typecheck!(CodeInstance<'scope>, jl_code_instance_type, 'scope);
impl_debug!(CodeInstance<'_>);

impl<'scope> WrapperPriv<'scope, '_> for CodeInstance<'scope> {
    type Wraps = jl_code_instance_t;
    type TypeConstructorPriv<'target, 'da> = CodeInstance<'target>;
    const NAME: &'static str = "CodeInstance";

    // Safety: `inner` must not have been freed yet, the result must never be
    // used after the GC might have freed it.
    unsafe fn wrap_non_null(inner: NonNull<Self::Wraps>, _: Private) -> Self {
        Self(inner, ::std::marker::PhantomData)
    }

    fn unwrap_non_null(self, _: Private) -> NonNull<Self::Wraps> {
        self.0
    }
}

/// A reference to a [`CodeInstance`] that has not been explicitly rooted.
pub type CodeInstanceRef<'scope> = Ref<'scope, 'static, CodeInstance<'scope>>;
impl_valid_layout!(CodeInstanceRef, CodeInstance);

use super::method_instance::MethodInstance;
use crate::memory::target::target_type::TargetType;

/// `CodeInstance` or `CodeInstanceRef`, depending on the target type `T`.
pub type CodeInstanceData<'target, T> =
    <T as TargetType<'target>>::Data<'static, CodeInstance<'target>>;

/// `JuliaResult<CodeInstance>` or `JuliaResultRef<CodeInstanceRef>`, depending on the target type
/// `T`.
pub type CodeInstanceResult<'target, T> =
    <T as TargetType<'target>>::Result<'static, CodeInstance<'target>>;
