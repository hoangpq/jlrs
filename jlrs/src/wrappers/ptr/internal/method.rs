//! Wrapper for `Method`.
//!
//! The documentation for this module has been slightly adapted from the comments for this struct
//! in [`julia.h`]
//!
//! [`julia.h`]: https://github.com/JuliaLang/julia/blob/96786e22ccabfdafd073122abb1fb69cea921e17/src/julia.h#L273

use std::{marker::PhantomData, ptr::NonNull};

use cfg_if::cfg_if;
use jl_sys::{jl_method_t, jl_method_type};

use crate::{
    impl_julia_typecheck,
    memory::target::Target,
    private::Private,
    wrappers::ptr::{
        array::{ArrayData, ArrayRef},
        internal::method_instance::MethodInstanceRef,
        module::{ModuleData, ModuleRef},
        private::WrapperPriv,
        simple_vector::{SimpleVectorData, SimpleVectorRef},
        symbol::Symbol,
        value::{ValueData, ValueRef},
        Ref,
    },
};

cfg_if! {
    if #[cfg(not(feature = "lts"))] {
        use std::sync::atomic::Ordering;
    }
}

#[cfg(not(feature = "lts"))]
use crate::wrappers::ptr::array::TypedArrayData;
#[cfg(not(feature = "lts"))]
use crate::wrappers::ptr::array::TypedArrayRef;

/// This type describes a single method definition, and stores data shared by the specializations
/// of a function.
#[derive(Copy, Clone)]
#[repr(transparent)]
pub struct Method<'scope>(NonNull<jl_method_t>, PhantomData<&'scope ()>);

impl<'scope> Method<'scope> {
    /*
    inspect(Core.Method):

    name: Symbol (mut)
    module: Module (mut)
    file: Symbol (mut)
    line: Int32 (mut)
    primary_world: UInt64 (mut)
    deleted_world: UInt64 (mut)
    sig: Type (mut)
    specializations: Core.SimpleVector (mut) _Atomic
    speckeyset: Array (mut) _Atomic
    slot_syms: String (mut)
    external_mt: Any (mut)
    source: Any (mut)
    unspecialized: Core.MethodInstance (mut) _Atomic
    generator: Any (mut)
    roots: Vector{Any} (mut)
    root_blocks: Vector{UInt64} (mut)
    nroots_sysimg: Int32 (mut)
    ccallable: Core.SimpleVector (mut)
    invokes: Any (mut) _Atomic
    recursion_relation: Any (mut)
    nargs: Int32 (mut)
    called: Int32 (mut)
    nospecialize: Int32 (mut)
    nkw: Int32 (mut)
    isva: Bool (mut)
    pure: Bool (mut)
    is_for_opaque_closure: Bool (mut)
    constprop: UInt8 (mut)
    purity: UInt8 (mut)
    */

    /// Method name for error reporting
    pub fn name(self) -> Option<Symbol<'scope>> {
        // Safety: the pointer points to valid data
        unsafe {
            let name = self.unwrap_non_null(Private).as_ref().name;
            let name = NonNull::new(name)?;
            Some(Symbol::wrap_non_null(name, Private))
        }
    }

    /// Method module
    pub fn module<'target, T>(self, target: T) -> Option<ModuleData<'target, T>>
    where
        T: Target<'target>,
    {
        // Safety: the pointer points to valid data
        unsafe {
            let module = self.unwrap_non_null(Private).as_ref().module;
            let module = NonNull::new(module)?;
            Some(ModuleRef::wrap(module).root(target))
        }
    }

    /// Method file
    pub fn file(self) -> Option<Symbol<'scope>> {
        // Safety: the pointer points to valid data
        unsafe {
            let file = self.unwrap_non_null(Private).as_ref().file;
            let file = NonNull::new(file)?;
            Some(Symbol::wrap_non_null(file, Private))
        }
    }

    /// Method line in file
    pub fn line(self) -> i32 {
        // Safety: the pointer points to valid data
        unsafe { self.unwrap_non_null(Private).as_ref().line }
    }

    /// The `primary_world` field.
    pub fn primary_world(self) -> usize {
        // Safety: the pointer points to valid data
        unsafe { self.unwrap_non_null(Private).as_ref().primary_world }
    }

    /// The `deleted_world` field.
    pub fn deleted_world(self) -> usize {
        // Safety: the pointer points to valid data
        unsafe { self.unwrap_non_null(Private).as_ref().deleted_world }
    }

    /// Method's type signature.
    pub fn signature<'target, T>(self, target: T) -> Option<ValueData<'target, 'static, T>>
    where
        T: Target<'target>,
    {
        // Safety: the pointer points to valid data
        unsafe {
            let sig = self.unwrap_non_null(Private).as_ref().sig;
            let sig = NonNull::new(sig)?;
            Some(ValueRef::wrap(sig).root(target))
        }
    }

    /// Table of all `Method` specializations, allocated as [hashable, ..., NULL, linear, ....]
    pub fn specializations<'target, T>(self, target: T) -> Option<SimpleVectorData<'target, T>>
    where
        T: Target<'target>,
    {
        cfg_if! {
            if #[cfg(feature = "lts")] {
                // Safety: the pointer points to valid data
                unsafe {
                    let specializations = self.unwrap_non_null(Private).as_ref().specializations;
                    let specializations = NonNull::new(specializations)?;
                    Some(SimpleVectorRef::wrap(specializations).root(target))
                }
            } else {
                // Safety: the pointer points to valid data
                unsafe {
                    let specializations = self.unwrap_non_null(Private).as_ref().specializations.load(Ordering::Relaxed);
                    let specializations = NonNull::new(specializations)?;
                    Some(SimpleVectorRef::wrap(specializations).root(target))
                }
            }
        }
    }

    /// Index lookup by hash into specializations
    pub fn spec_key_set<'target, T>(self, target: T) -> Option<ArrayData<'target, 'static, T>>
    where
        T: Target<'target>,
    {
        cfg_if! {
            if #[cfg(feature = "lts")] {
                // Safety: the pointer points to valid data
                unsafe {
                    let speckeyset = self.unwrap_non_null(Private).as_ref().speckeyset;
                    let speckeyset = NonNull::new(speckeyset)?;
                    Some(ArrayRef::wrap(speckeyset).root(target))
                }
            } else {
                // Safety: the pointer points to valid data
                unsafe {
                    let speckeyset = self.unwrap_non_null(Private).as_ref().speckeyset.load(Ordering::Relaxed);
                    let speckeyset = NonNull::new(speckeyset)?;
                    Some(ArrayRef::wrap(speckeyset).root(target))
                }
            }
        }
    }

    /// Compacted list of slot names (String)
    pub fn slot_syms<'target, T>(self, target: T) -> Option<ValueData<'target, 'static, T>>
    where
        T: Target<'target>,
    {
        // Safety: the pointer points to valid data
        unsafe {
            let slot_syms = self.unwrap_non_null(Private).as_ref().slot_syms;
            let slot_syms = NonNull::new(slot_syms)?;
            Some(ValueRef::wrap(slot_syms).root(target))
        }
    }

    /// reference to the method table this method is part of, null if part of the internal table
    #[cfg(not(feature = "lts"))]
    pub fn external_mt(self) -> Option<ValueRef<'scope, 'static>> {
        // Safety: the pointer points to valid data
        unsafe {
            let data = self.unwrap_non_null(Private).as_ref().external_mt;
            let data = NonNull::new(data)?;
            Some(ValueRef::wrap(data))
        }
    }

    // Original code template (`Core.CodeInfo`, but may be compressed), `None` for builtins.
    pub fn source<'target, T>(self, target: T) -> Option<ValueData<'target, 'static, T>>
    where
        T: Target<'target>,
    {
        // Safety: the pointer points to valid data
        unsafe {
            let data = self.unwrap_non_null(Private).as_ref().source;
            let data = NonNull::new(data)?;
            Some(ValueRef::wrap(data).root(target))
        }
    }

    /// Unspecialized executable method instance, or `None`
    pub fn unspecialized<'target, T>(self, target: T) -> Option<MethodInstanceData<'target, T>>
    where
        T: Target<'target>,
    {
        cfg_if! {
            if #[cfg(feature = "lts")] {
                // Safety: the pointer points to valid data
                unsafe {
                    let unspecialized = self.unwrap_non_null(Private).as_ref().unspecialized;
                    let unspecialized = NonNull::new(unspecialized)?;
                    Some(MethodInstanceRef::wrap(unspecialized).root(target))
                }
            } else {
                // Safety: the pointer points to valid data
                unsafe {
                    let unspecialized = self.unwrap_non_null(Private).as_ref().unspecialized.load(Ordering::Relaxed);
                    let unspecialized = NonNull::new(unspecialized)?;
                    Some(MethodInstanceRef::wrap(unspecialized).root(target))
                }
            }
        }
    }

    /// Executable code-generating function if available
    pub fn generator<'target, T>(self, target: T) -> Option<ValueData<'target, 'static, T>>
    where
        T: Target<'target>,
    {
        // Safety: the pointer points to valid data
        unsafe {
            let data = self.unwrap_non_null(Private).as_ref().generator;
            let data = NonNull::new(data)?;
            Some(ValueRef::wrap(data).root(target))
        }
    }

    /// Pointers in generated code (shared to reduce memory), or `None`
    pub fn roots<'target, T>(self, target: T) -> Option<ArrayData<'target, 'static, T>>
    where
        T: Target<'target>,
    {
        // Safety: the pointer points to valid data
        unsafe {
            let data = self.unwrap_non_null(Private).as_ref().roots;
            let data = NonNull::new(data)?;
            Some(ArrayRef::wrap(data).root(target))
        }
    }

    /// RLE (build_id, offset) pairs (even/odd indexing)
    #[cfg(not(feature = "lts"))]
    pub fn root_blocks<'target, T>(
        self,
        target: T,
    ) -> Option<TypedArrayData<'target, 'static, T, u64>>
    where
        T: Target<'target>,
    {
        // Safety: the pointer points to valid data

        unsafe {
            let data = self.unwrap_non_null(Private).as_ref().root_blocks;
            let data = NonNull::new(data)?;
            Some(TypedArrayRef::wrap(data).root(target))
        }
    }

    /// # of roots stored in the system image
    #[cfg(not(feature = "lts"))]
    pub fn nroots_sysimg(self) -> i32 {
        // Safety: the pointer points to valid data
        unsafe { self.unwrap_non_null(Private).as_ref().nroots_sysimg }
    }

    /// `SimpleVector(rettype, sig)` if a ccallable entry point is requested for this
    pub fn ccallable<'target, T>(self, target: T) -> Option<SimpleVectorData<'target, T>>
    where
        T: Target<'target>,
    {
        // Safety: the pointer points to valid data
        unsafe {
            let data = self.unwrap_non_null(Private).as_ref().ccallable;
            let data = NonNull::new(data)?;
            Some(SimpleVectorRef::wrap(data).root(target))
        }
    }

    /// Cache of specializations of this method for invoke(), i.e.
    /// cases where this method was called even though it was not necessarily
    /// the most specific for the argument types.
    pub fn invokes<'target, T>(self, target: T) -> Option<ValueData<'target, 'static, T>>
    where
        T: Target<'target>,
    {
        cfg_if! {
            if #[cfg(feature = "lts")] {
                // Safety: the pointer points to valid data
                unsafe {
                    let invokes = self.unwrap_non_null(Private).as_ref().invokes;
                    let invokes = NonNull::new(invokes)?;
                    Some(ValueRef::wrap(invokes).root(target))
                }
            } else {
                // Safety: the pointer points to valid data
                unsafe {
                    let invokes = self.unwrap_non_null(Private).as_ref().invokes.load(Ordering::Relaxed);
                    let invokes = NonNull::new(invokes)?;
                    Some(ValueRef::wrap(invokes).root(target))
                }
            }
        }
    }

    /// The `n_args` field.
    pub fn n_args(self) -> u32 {
        // Safety: the pointer points to valid data
        unsafe { self.unwrap_non_null(Private).as_ref().nargs as u32 }
    }

    /// Bit flags: whether each of the first 8 arguments is called
    pub fn called(self) -> u32 {
        // Safety: the pointer points to valid data
        unsafe { self.unwrap_non_null(Private).as_ref().called as u32 }
    }

    /// Bit flags: which arguments should not be specialized
    pub fn no_specialize(self) -> u32 {
        // Safety: the pointer points to valid data
        unsafe { self.unwrap_non_null(Private).as_ref().nospecialize as u32 }
    }

    /// Number of leading arguments that are actually keyword arguments
    /// of another method.
    pub fn nkw(self) -> u32 {
        // Safety: the pointer points to valid data
        unsafe { self.unwrap_non_null(Private).as_ref().nkw as u32 }
    }

    /// The `isva` field.
    pub fn is_varargs(self) -> bool {
        // Safety: the pointer points to valid data
        unsafe { self.unwrap_non_null(Private).as_ref().isva != 0 }
    }

    /// The `pure` field.
    pub fn pure(self) -> bool {
        // Safety: the pointer points to valid data
        unsafe { self.unwrap_non_null(Private).as_ref().pure_ != 0 }
    }

    /// The `is_for_opaque_closure` field of this `Method`
    #[cfg(not(feature = "lts"))]
    pub fn is_for_opaque_closure(self) -> bool {
        // Safety: the pointer points to valid data
        unsafe { self.unwrap_non_null(Private).as_ref().is_for_opaque_closure != 0 }
    }

    /// 0x00 = use heuristic; 0x01 = aggressive; 0x02 = none
    #[cfg(not(feature = "lts"))]
    pub fn constprop(self) -> u8 {
        // Safety: the pointer points to valid data
        unsafe { self.unwrap_non_null(Private).as_ref().constprop }
    }

    /// Override the conclusions of inter-procedural effect analysis,
    /// forcing the conclusion to always true.
    #[cfg(not(feature = "lts"))]
    pub fn purity(self) -> u8 {
        // Safety: the pointer points to valid data
        unsafe { self.unwrap_non_null(Private).as_ref().purity.bits }
    }
}

impl_julia_typecheck!(Method<'scope>, jl_method_type, 'scope);
impl_debug!(Method<'_>);

impl<'scope> WrapperPriv<'scope, '_> for Method<'scope> {
    type Wraps = jl_method_t;
    type TypeConstructorPriv<'target, 'da> = Method<'target>;
    const NAME: &'static str = "Method";

    // Safety: `inner` must not have been freed yet, the result must never be
    // used after the GC might have freed it.
    unsafe fn wrap_non_null(inner: NonNull<Self::Wraps>, _: Private) -> Self {
        Self(inner, ::std::marker::PhantomData)
    }

    fn unwrap_non_null(self, _: Private) -> NonNull<Self::Wraps> {
        self.0
    }
}

/// A reference to a [`Method`] that has not been explicitly rooted.
pub type MethodRef<'scope> = Ref<'scope, 'static, Method<'scope>>;
impl_valid_layout!(MethodRef, Method);

use super::method_instance::MethodInstanceData;
use crate::memory::target::target_type::TargetType;

/// `Method` or `MethodRef`, depending on the target type `T`.
pub type MethodData<'target, T> = <T as TargetType<'target>>::Data<'static, Method<'target>>;

/// `JuliaResult<Method>` or `JuliaResultRef<MethodRef>`, depending on the target type `T`.
pub type MethodResult<'target, T> = <T as TargetType<'target>>::Result<'static, Method<'target>>;
