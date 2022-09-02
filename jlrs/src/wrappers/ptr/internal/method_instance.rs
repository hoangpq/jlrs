//! Wrapper for `MethodInstance`.
//!
//! The documentation for this module has been slightly adapted from the comments for this struct
//! in [`julia.h`]
//!
//! [`julia.h`]: https://github.com/JuliaLang/julia/blob/96786e22ccabfdafd073122abb1fb69cea921e17/src/julia.h#L321
use crate::{
    impl_julia_typecheck,
    memory::output::Output,
    private::Private,
    wrappers::ptr::{
        internal::code_instance::CodeInstanceRef, private::WrapperPriv,
        simple_vector::SimpleVectorRef, value::ValueRef, Ref,
    },
};
use cfg_if::cfg_if;
use jl_sys::{jl_method_instance_t, jl_method_instance_type};
use std::{marker::PhantomData, ptr::NonNull};

cfg_if! {
    if #[cfg(any(not(feature = "lts"), feature = "all-features-override"))] {
        use std::sync::atomic::Ordering;
    }
}

/// This type is a placeholder to cache data for a specType signature specialization of a `Method`
/// can can be used as a unique dictionary key representation of a call to a particular `Method`
/// with a particular set of argument types.
#[derive(Copy, Clone)]
#[repr(transparent)]
pub struct MethodInstance<'scope>(NonNull<jl_method_instance_t>, PhantomData<&'scope ()>);

impl<'scope> MethodInstance<'scope> {
    /*
    for (a, b) in zip(fieldnames(Core.MethodInstance), fieldtypes(Core.MethodInstance))
        println(a, ": ", b)
    end
    def: Union{Method, Module}
    specTypes: Any
    sparam_vals: Core.SimpleVector
    uninferred: Any
    backedges: Any
    callbacks: Any
    cache: Core.CodeInstance _Atomic
    inInference: Bool
    precompiled: Bool
    */

    /// pointer back to the context for this code
    pub fn def(self) -> ValueRef<'scope, 'static> {
        // Safety: the pointer points to valid data
        unsafe { ValueRef::wrap(self.unwrap_non_null(Private).as_ref().def.value) }
    }

    /// Argument types this was specialized for
    pub fn spec_types(self) -> ValueRef<'scope, 'static> {
        // Safety: the pointer points to valid data
        unsafe { ValueRef::wrap(self.unwrap_non_null(Private).as_ref().specTypes) }
    }

    /// Static parameter values, indexed by def.method->sparam_syms
    pub fn sparam_vals(self) -> SimpleVectorRef<'scope> {
        // Safety: the pointer points to valid data
        unsafe { SimpleVectorRef::wrap(self.unwrap_non_null(Private).as_ref().sparam_vals) }
    }

    /// Cached uncompressed code, for generated functions, top-level thunks, or the interpreter
    pub fn uninferred(self) -> ValueRef<'scope, 'static> {
        // Safety: the pointer points to valid data
        unsafe { ValueRef::wrap(self.unwrap_non_null(Private).as_ref().uninferred) }
    }

    /// List of method-instances which contain a call into this method-instance
    pub fn backedges(self) -> ValueRef<'scope, 'static> {
        // Safety: the pointer points to valid data
        unsafe { ValueRef::wrap(self.unwrap_non_null(Private).as_ref().backedges.cast()) }
    }

    /// list of callback functions to inform external caches about invalidations
    pub fn callbacks(self) -> ValueRef<'scope, 'static> {
        // Safety: the pointer points to valid data
        unsafe { ValueRef::wrap(self.unwrap_non_null(Private).as_ref().callbacks.cast()) }
    }

    /// The `cache` field.
    pub fn cache(self) -> CodeInstanceRef<'scope> {
        cfg_if! {
            if #[cfg(all(feature = "lts", not(feature = "all-features-override")))] {
                // Safety: the pointer points to valid data
                unsafe { CodeInstanceRef::wrap(self.unwrap_non_null(Private).as_ref().cache) }
            } else {
                // Safety: the pointer points to valid data
                unsafe {
                    let cache = self.unwrap_non_null(Private).as_ref().cache.load(Ordering::Relaxed);
                    CodeInstanceRef::wrap(cache)
                }
            }
        }
    }

    /// Flags to tell if inference is running on this object
    pub fn in_inference(self) -> bool {
        // Safety: the pointer points to valid data
        unsafe { self.unwrap_non_null(Private).as_ref().inInference != 0 }
    }

    /// `true` if this instance was generated by an explicit `precompile(...)` call
    #[cfg(any(not(feature = "lts"), feature = "all-features-override"))]
    pub fn precompiled(self) -> bool {
        // Safety: the pointer points to valid data
        unsafe { self.unwrap_non_null(Private).as_ref().precompiled != 0 }
    }

    /// Use the `Output` to extend the lifetime of this data.
    pub fn root<'target>(self, output: Output<'target>) -> MethodInstance<'target> {
        // Safety: the pointer points to valid data
        unsafe {
            let ptr = self.unwrap_non_null(Private);
            output.set_root::<MethodInstance>(ptr);
            MethodInstance::wrap_non_null(ptr, Private)
        }
    }
}

impl_julia_typecheck!(MethodInstance<'scope>, jl_method_instance_type, 'scope);
impl_debug!(MethodInstance<'_>);

impl<'scope> WrapperPriv<'scope, '_> for MethodInstance<'scope> {
    type Wraps = jl_method_instance_t;
    const NAME: &'static str = "MethodInstance";

    // Safety: `inner` must not have been freed yet, the result must never be
    // used after the GC might have freed it.
    unsafe fn wrap_non_null(inner: NonNull<Self::Wraps>, _: Private) -> Self {
        Self(inner, PhantomData)
    }

    fn unwrap_non_null(self, _: Private) -> NonNull<Self::Wraps> {
        self.0
    }
}

impl_root!(MethodInstance, 1);
/// A reference to a [`MethodInstance`] that has not been explicitly rooted.
pub type MethodInstanceRef<'scope> = Ref<'scope, 'static, MethodInstance<'scope>>;
impl_valid_layout!(MethodInstanceRef, MethodInstance);
impl_ref_root!(MethodInstance, MethodInstanceRef, 1);