//! Wrapper for `Module`, which provides access to Julia's modules and their contents.
//!
//! In Julia, each module introduces a separate global scope. There are three important "root"
//! modules, `Main`, `Base` and `Core`. Any Julia code that you include in jlrs is made available
//! relative to the `Main` module.

use std::{marker::PhantomData, ptr::NonNull};

use jl_sys::{
    jl_base_module,
    jl_core_module,
    jl_get_global,
    jl_is_imported,
    jl_main_module,
    jl_module_t,
    jl_module_type,
    jl_set_const,
    jl_set_global,
};

use super::{
    function::FunctionData,
    value::{ValueData, ValueResult},
    Ref,
};
use crate::{
    call::Call,
    convert::to_symbol::ToSymbol,
    error::{AccessError, JlrsResult, TypeError},
    impl_julia_typecheck,
    memory::target::Target,
    private::Private,
    wrappers::ptr::{
        function::Function,
        private::WrapperPriv,
        symbol::Symbol,
        value::{LeakedValue, Value},
        Wrapper as _,
    },
};

/// Functionality in Julia can be accessed through its module system. You can get a handle to the
/// three standard modules, `Main`, `Base`, and `Core` and access their submodules through them.
/// If you include your own Julia code with [`Julia::include`], [`AsyncJulia::include`], or
/// [`AsyncJulia::try_include`] its contents are made available relative to `Main`.
///
/// The most important methods offered by this wrapper are those that let you access submodules,
/// functions, and other global values defined in the module. These come in two variants: one that
/// roots the result and one that doesn't. If you never redefine the module, it's safe to leave
/// named functions, constants and submodules unrooted when you use them from Rust. The same holds
/// true for other global values that are never redefined to point at another value.
///
/// [`Julia::include`]: crate::runtime::sync_rt::Julia::include
/// [`AsyncJulia::include`]: crate::runtime::async_rt::AsyncJulia::include
/// [`AsyncJulia::try_include`]: crate::runtime::async_rt::AsyncJulia::try_include
#[derive(Copy, Clone, PartialEq)]
#[repr(transparent)]
pub struct Module<'scope>(NonNull<jl_module_t>, PhantomData<&'scope ()>);

impl<'scope> Module<'scope> {
    /// Returns the name of this module.
    pub fn name(self) -> Symbol<'scope> {
        // Safety: the pointer points to valid data, the name is never null
        unsafe {
            let sym = NonNull::new_unchecked(self.unwrap_non_null(Private).as_ref().name);
            Symbol::wrap_non_null(sym, Private)
        }
    }

    /// Returns the parent of this module.
    pub fn parent(self) -> Module<'scope> {
        // Safety: the pointer points to valid data, the parent is never null
        unsafe {
            let parent = self.unwrap_non_null(Private).as_ref().parent;
            Module(NonNull::new_unchecked(parent), PhantomData)
        }
    }

    /// Extend the lifetime of this module. This is safe as long as the module is never redefined.
    pub unsafe fn extend<'target, T>(self, _: &T) -> Module<'target>
    where
        T: Target<'target>,
    {
        Module::wrap_non_null(self.unwrap_non_null(Private), Private)
    }

    /// Returns a handle to Julia's `Main`-module. If you include your own Julia code with
    /// [`Julia::include`], [`AsyncJulia::include`], or [`AsyncJulia::try_include`] its contents
    ///  are made available relative to `Main`.
    ///
    /// [`Julia::include`]: crate::runtime::sync_rt::Julia::include
    /// [`AsyncJulia::include`]: crate::runtime::async_rt::AsyncJulia::include
    /// [`AsyncJulia::try_include`]: crate::runtime::async_rt::AsyncJulia::try_include
    pub fn main<T: Target<'scope>>(_: &T) -> Self {
        // Safety: the Main module is globally rooted
        unsafe { Module::wrap_non_null(NonNull::new_unchecked(jl_main_module), Private) }
    }

    /// Returns a handle to Julia's `Core`-module.
    pub fn core<T: Target<'scope>>(_: &T) -> Self {
        // Safety: the Core module is globally rooted
        unsafe { Module::wrap_non_null(NonNull::new_unchecked(jl_core_module), Private) }
    }

    /// Returns a handle to Julia's `Base`-module.
    pub fn base<T: Target<'scope>>(_: &T) -> Self {
        // Safety: the Base module is globally rooted
        unsafe { Module::wrap_non_null(NonNull::new_unchecked(jl_base_module), Private) }
    }

    /// Returns `true` if `self` has imported `sym`.
    pub fn is_imported<N: ToSymbol>(self, sym: N) -> bool {
        // Safety: the pointer points to valid data, the C API function is called with
        // valid arguments.
        unsafe {
            let sym = sym.to_symbol_priv(Private);
            jl_is_imported(self.unwrap(Private), sym.unwrap(Private)) != 0
        }
    }

    /// Returns the submodule named `name` relative to this module. You have to visit this level
    /// by level: you can't access `Main.A.B` by calling this function with `"A.B"`, but have to
    /// access `A` first and then `B`.
    ///
    /// Returns an error if the submodule doesn't exist.
    pub fn submodule<'target, N, T>(self, target: T, name: N) -> JlrsResult<ModuleData<'target, T>>
    where
        N: ToSymbol,
        T: Target<'target>,
    {
        // Safety: the pointer points to valid data, the C API function is called with
        // valid arguments and its result is checked.
        unsafe {
            let symbol = name.to_symbol_priv(Private);
            let submodule = jl_get_global(self.unwrap(Private), symbol.unwrap(Private));
            if submodule.is_null() {
                Err(AccessError::GlobalNotFound {
                    name: symbol.as_str().unwrap_or("<Non-UTF8 symbol>").into(),
                    module: self.name().as_str().unwrap_or("<Non-UTF8 symbol>").into(),
                })?
            }

            let submodule_nn = NonNull::new_unchecked(submodule);
            let submodule_v = Value::wrap_non_null(submodule_nn, Private);
            if !submodule_v.is::<Self>() {
                Err(TypeError::NotAModule {
                    name: symbol.as_str().unwrap_or("<Non-UTF8 symbol>").into(),
                    ty: submodule_v.datatype().name().into(),
                })?
            }

            Ok(target.data_from_ptr(submodule_nn.cast(), Private))
        }
    }

    /// Set a global value in this module. Note that if this global already exists, this can
    /// make the old value unreachable. If an excection is thrown, it's caught, rooted and
    /// returned.
    ///
    /// Safety: Mutating Julia data is generally unsafe because it can't be guaranteed mutating
    /// this value is allowed.
    #[cfg(not(all(target_os = "windows", feature = "lts")))]
    pub unsafe fn set_global<'target, N, T>(
        self,
        target: T,
        name: N,
        value: Value<'_, 'static>,
    ) -> T::Exception<'static, ()>
    where
        N: ToSymbol,
        T: Target<'target>,
    {
        use std::mem::MaybeUninit;

        use crate::catch::catch_exceptions;
        let symbol = name.to_symbol_priv(Private);

        let mut callback = |result: &mut MaybeUninit<()>| {
            jl_set_global(
                self.unwrap(Private),
                symbol.unwrap(Private),
                value.unwrap(Private),
            );

            result.write(());
            Ok(())
        };

        let res = match catch_exceptions(&mut callback).unwrap() {
            Ok(_) => Ok(()),
            Err(e) => Err(e.ptr()),
        };

        target.exception_from_ptr(res, Private)
    }

    /// Set a global value in this module. Note that if this global already exists, this can
    /// make the old value unreachable.
    ///
    /// Safety: Mutating Julia data is generally unsafe because it can't be guaranteed mutating
    /// this value is allowed.
    pub unsafe fn set_global_unchecked<N>(self, name: N, value: Value<'_, 'static>)
    where
        N: ToSymbol,
    {
        let symbol = name.to_symbol_priv(Private);

        jl_set_global(
            self.unwrap(Private),
            symbol.unwrap(Private),
            value.unwrap(Private),
        );
    }

    /// Set a constant in this module. If Julia throws an exception it's caught and rooted in the
    /// current frame, if the exception can't be rooted a `JlrsError::AllocError` is returned. If
    /// no exception is thrown an unrooted reference to the constant is returned.
    #[cfg(not(all(target_os = "windows", feature = "lts")))]
    pub fn set_const<'target, N, T>(
        self,
        target: T,
        name: N,
        value: Value<'_, 'static>,
    ) -> T::Exception<'static, Value<'scope, 'static>>
    where
        N: ToSymbol,
        T: Target<'target>,
    {
        // Safety: the pointer points to valid data, the C API function is called with
        // valid arguments and its result is checked. if an exception is thrown it's caught
        // and returned
        unsafe {
            use std::mem::MaybeUninit;

            use crate::catch::catch_exceptions;
            let symbol = name.to_symbol_priv(Private);

            let mut callback = |result: &mut MaybeUninit<()>| {
                jl_set_const(
                    self.unwrap(Private),
                    symbol.unwrap(Private),
                    value.unwrap(Private),
                );

                result.write(());
                Ok(())
            };

            let res = match catch_exceptions(&mut callback).unwrap() {
                Ok(_) => Ok(Value::wrap_non_null(
                    value.unwrap_non_null(Private),
                    Private,
                )),
                Err(e) => Err(e.ptr()),
            };

            target.exception_from_ptr(res, Private)
        }
    }

    /// Set a constant in this module. If the constant already exists the process aborts,
    /// otherwise an unrooted reference to the constant is returned.
    ///
    /// Safety: This method must not throw an error if called from a `ccall`ed function.
    pub unsafe fn set_const_unchecked<N>(
        self,
        name: N,
        value: Value<'_, 'static>,
    ) -> Value<'scope, 'static>
    where
        N: ToSymbol,
    {
        let symbol = name.to_symbol_priv(Private);

        jl_set_const(
            self.unwrap(Private),
            symbol.unwrap(Private),
            value.unwrap(Private),
        );

        Value::wrap_non_null(value.unwrap_non_null(Private), Private)
    }

    /// Returns the global named `name` in this module.
    /// Returns an error if the global doesn't exist.
    pub fn global<'target, N, T>(
        self,
        target: T,
        name: N,
    ) -> JlrsResult<ValueData<'target, 'static, T>>
    where
        N: ToSymbol,
        T: Target<'target>,
    {
        // Safety: the pointer points to valid data, the C API function is called with
        // valid arguments and its result is checked.
        unsafe {
            let symbol = name.to_symbol_priv(Private);

            let global = jl_get_global(self.unwrap(Private), symbol.unwrap(Private));
            if global.is_null() {
                Err(AccessError::GlobalNotFound {
                    name: symbol.as_str().unwrap_or("<Non-UTF8 symbol>").into(),
                    module: self.name().as_str().unwrap_or("<Non-UTF8 symbol>").into(),
                })?;
            }

            Ok(target.data_from_ptr(NonNull::new_unchecked(global), Private))
        }
    }

    /// Returns the global named `name` in this module as a [`LeakedValue`].
    /// Returns an error if the global doesn't exist.
    pub fn leaked_global<N>(self, name: N) -> JlrsResult<LeakedValue>
    where
        N: ToSymbol,
    {
        // Safety: the pointer points to valid data, the C API function is called with
        // valid arguments and its result is checked.
        unsafe {
            let symbol = name.to_symbol_priv(Private);

            let global = jl_get_global(self.unwrap(Private), symbol.unwrap(Private));
            if global.is_null() {
                Err(AccessError::GlobalNotFound {
                    name: symbol.as_str().unwrap_or("<Non-UTF8 symbol>").into(),
                    module: self.name().as_str().unwrap_or("<Non-UTF8 symbol>").into(),
                })?;
            }

            Ok(LeakedValue::wrap_non_null(NonNull::new_unchecked(global)))
        }
    }

    /// Returns the function named `name` in this module.
    /// Returns an error if the function doesn't exist or if it's not a subtype of `Function`.
    pub fn function<'target, N, T>(
        self,
        target: T,
        name: N,
    ) -> JlrsResult<FunctionData<'target, 'static, T>>
    where
        N: ToSymbol,
        T: Target<'target>,
    {
        // Safety: the pointer points to valid data, the result is checked.
        unsafe {
            let symbol = name.to_symbol_priv(Private);
            let func = self.global(&target, symbol)?.wrapper();

            if !func.is::<Function>() {
                let name = symbol.as_str().unwrap_or("<Non-UTF8 string>").into();
                let ty = func.datatype_name().unwrap_or("<Non-UTF8 string>").into();
                Err(TypeError::NotAFunction { name, ty: ty })?;
            }

            Ok(target.data_from_ptr(func.unwrap_non_null(Private).cast(), Private))
        }
    }

    /// Convert `self` to a `LeakedValue`.
    pub fn as_leaked(self) -> LeakedValue {
        // Safety: the pointer points to valid data
        unsafe { LeakedValue::wrap_non_null(self.unwrap_non_null(Private).cast()) }
    }

    /// Load a module by calling `Base.require` and return this module if it has been loaded
    /// successfully. This method can be used to load parts of the standard library like
    /// `LinearAlgebra`. This requires one slot on the GC stack. Note that the loaded module is
    /// not made available in the module used to call this method, you can use
    /// `Module::set_global` to do so.
    ///
    /// Note that when you want to call `using Submodule` in the `Main` module, you can do so by
    /// evaluating the using-statement with [`Value::eval_string`].
    ///
    /// Safety: This method can execute arbitrary Julia code depending on the module that is
    /// loaded.
    pub unsafe fn require<'target, T, N>(
        self,
        target: T,
        module: N,
    ) -> ValueResult<'target, 'static, T>
    where
        T: Target<'target>,
        N: ToSymbol,
    {
        Module::base(&target)
            .function(&target, "require")
            .unwrap()
            .wrapper()
            .call2(
                target,
                self.as_value(),
                module.to_symbol_priv(Private).as_value(),
            )
    }
}

impl_julia_typecheck!(Module<'target>, jl_module_type, 'target);
impl_debug!(Module<'_>);

impl<'scope> WrapperPriv<'scope, '_> for Module<'scope> {
    type Wraps = jl_module_t;
    type TypeConstructorPriv<'target, 'da> = Module<'target>;
    const NAME: &'static str = "Module";

    // Safety: `inner` must not have been freed yet, the result must never be
    // used after the GC might have freed it.
    unsafe fn wrap_non_null(inner: NonNull<Self::Wraps>, _: Private) -> Self {
        Self(inner, PhantomData)
    }

    fn unwrap_non_null(self, _: Private) -> NonNull<Self::Wraps> {
        self.0
    }
}

/// A reference to a [`Module`] that has not been explicitly rooted.
pub type ModuleRef<'scope> = Ref<'scope, 'static, Module<'scope>>;
impl_valid_layout!(ModuleRef, Module);

use crate::memory::target::target_type::TargetType;

/// `Module` or `ModuleRef`, depending on the target type `T`.
pub type ModuleData<'target, T> = <T as TargetType<'target>>::Data<'static, Module<'target>>;

/// `JuliaResult<Module>` or `JuliaResultRef<ModuleRef>`, depending on the target type `T`.
pub type ModuleResult<'target, T> = <T as TargetType<'target>>::Result<'static, Module<'target>>;
