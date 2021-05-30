//! Wrapper for `Core.Module`, which provides access to Julia's modules and their contents.
//!
//! In Julia, each module introduces a separate global scope. There are three important "root"
//! modules, `Main`, `Base` and `Core`. Any Julia code that you include in jlrs is made available
//! relative to the `Main` module, just like in Julia itself.

use crate::{
    convert::temporary_symbol::TemporarySymbol,
    error::{JlrsError, JlrsResult},
    impl_julia_typecheck, impl_valid_layout,
    memory::{
        global::Global,
        traits::{frame::Frame, scope::Scope},
    },
    prelude::Call,
    private::Private,
    wrappers::ptr::{
        function::Function,
        symbol::Symbol,
        value::{LeakedValue, Value},
        FunctionRef, ModuleRef, ValueRef, Wrapper as _,
    },
};
use jl_sys::{
    jl_base_module, jl_core_module, jl_get_global, jl_main_module, jl_module_t, jl_module_type,
    jl_set_const, jl_set_global, jl_typeis,
};
use std::fmt::{Debug, Formatter, Result as FmtResult};
use std::marker::PhantomData;
use std::ptr::NonNull;

use super::private::Wrapper;

/// Functionality in Julia can be accessed through its module system. You can get a handle to the
/// three standard modules, `Main`, `Base`, and `Core` and access their submodules through them.
/// If you include your own Julia code with [`Julia::include`], its contents are made available
/// relative to `Main`.
///
/// This struct can be used in combination with [`DataType::is`] and [`Value::is`]; if the check
/// returns `true` the [`Value`] can be cast to `Module`.
///
/// The most important methods offered by this struct are those that let you access submodules,
/// functions, and other global values defined in the module. These come in two flavors: one that
/// roots the result and one that doesn't. If you never redefine the module, it's safe to leave
/// named functions, constants and submodules unrooted when you use them from Rust. The same holds
/// true for other global values that are never redefined to point at another value.
///
/// [`Julia::include`]: crate::Julia::include
/// [`DataType::is`]: crate::wrappers::builtin::datatype::DataType::is
#[derive(Copy, Clone)]
#[repr(transparent)]
pub struct Module<'scope>(NonNull<jl_module_t>, PhantomData<&'scope ()>);

impl<'scope> Module<'scope> {
    /// Returns the name of this module.
    pub fn name(self) -> Symbol<'scope> {
        unsafe {
            let sym = NonNull::new_unchecked(self.unwrap_non_null(Private).as_ref().name);
            Symbol::wrap_non_null(sym, Private)
        }
    }

    /// Returns the parent of this module.
    pub fn parent<'frame, F>(self, frame: &mut F) -> JlrsResult<Module<'frame>>
    where
        F: Frame<'frame>,
    {
        unsafe {
            let parent = self.unwrap_non_null(Private).as_ref().parent;
            frame
                .push_root(NonNull::new_unchecked(parent.cast()), Private)
                .map(|v| v.cast_unchecked::<Module>())
                .map_err(Into::into)
        }
    }

    /// Returns the parent of this module without rooting it.
    pub fn parent_ref(self) -> ModuleRef<'scope> {
        unsafe { ModuleRef::wrap(self.unwrap_non_null(Private).as_ref().parent) }
    }

    /// Extend the lifetime of this module. This is safe as long as the module is never redefined.
    pub unsafe fn extend<'global>(self, _: Global<'global>) -> Module<'global> {
        Module::wrap(self.unwrap(Private), Private)
    }

    /// Returns a handle to Julia's `Main`-module. If you include your own Julia code by calling
    /// [`Julia::include`], handles to functions, globals, and submodules defined in these
    /// included files are available through this module.
    ///
    /// [`Julia::include`]: crate::Julia::include
    pub fn main(_: Global<'scope>) -> Self {
        unsafe { Module::wrap(jl_main_module, Private) }
    }

    /// Returns a handle to Julia's `Core`-module.
    pub fn core(_: Global<'scope>) -> Self {
        unsafe { Module::wrap(jl_core_module, Private) }
    }

    /// Returns a handle to Julia's `Base`-module.
    pub fn base(_: Global<'scope>) -> Self {
        unsafe { Module::wrap(jl_base_module, Private) }
    }

    /// Returns the submodule named `name` relative to this module. You have to visit this level
    /// by level: you can't access `Main.A.B` by calling this function with `"A.B"`, but have to
    /// access `A` first and then `B`.
    ///
    /// Returns an error if the submodule doesn't exist.
    pub fn submodule<'frame, N, F>(self, frame: &mut F, name: N) -> JlrsResult<Module<'frame>>
    where
        N: TemporarySymbol,
        F: Frame<'frame>,
    {
        unsafe {
            let symbol = name.temporary_symbol(Private);
            let submodule = jl_get_global(self.unwrap(Private), symbol.unwrap(Private));

            if !submodule.is_null() && jl_typeis(submodule, jl_module_type) {
                frame
                    .push_root(NonNull::new_unchecked(submodule), Private)
                    .map(|v| v.cast_unchecked())
                    .map_err(Into::into)
            } else {
                Err(JlrsError::NotAModule(
                    symbol.as_str().unwrap_or("<Non-UTF8 symbol>").into(),
                ))?
            }
        }
    }

    /// Returns the submodule named `name` relative to this module. You have to visit this level
    /// by level: you can't access `Main.A.B` by calling this function with `"A.B"`, but have to
    /// access `A` first and then `B`.
    ///
    /// Returns an error if the submodule doesn't exist.
    pub fn submodule_ref<N>(self, name: N) -> JlrsResult<ModuleRef<'scope>>
    where
        N: TemporarySymbol,
    {
        unsafe {
            let symbol = name.temporary_symbol(Private);
            let submodule = jl_get_global(self.unwrap(Private), symbol.unwrap(Private));

            if !submodule.is_null() && jl_typeis(submodule, jl_module_type) {
                Ok(ModuleRef::wrap(submodule.cast()))
            } else {
                Err(JlrsError::NotAModule(
                    symbol.as_str().unwrap_or("<Non-UTF8 symbol>").into(),
                ))?
            }
        }
    }

    /// Set a global value in this module. This is unsafe because if another global value was
    /// previously assigned to this name, this previous value can become eligible for garbage
    /// collection. Don't use the previous value after calling this method.
    pub fn set_global<'frame, N>(
        self,
        name: N,
        value: Value<'frame, 'static>,
    ) -> ValueRef<'scope, 'static>
    where
        N: TemporarySymbol,
    {
        unsafe {
            jl_set_global(
                self.unwrap(Private),
                name.temporary_symbol(Private).unwrap(Private),
                value.unwrap(Private),
            );
            ValueRef::wrap(value.unwrap(Private))
        }
    }

    /// Set a constant in this module.
    pub fn set_const<'frame, N>(
        self,
        name: N,
        value: Value<'frame, 'static>,
    ) -> JlrsResult<ValueRef<'scope, 'static>>
    where
        N: TemporarySymbol,
    {
        unsafe {
            let symbol = name.temporary_symbol(Private);
            if self.global_ref(symbol).is_ok() {
                Err(JlrsError::ConstAlreadyExists(
                    symbol.as_str().unwrap_or("<Non-UTF8 symbol>").into(),
                ))?;
            }

            jl_set_const(
                self.unwrap(Private),
                symbol.unwrap(Private),
                value.unwrap(Private),
            );

            Ok(ValueRef::wrap(value.unwrap(Private)))
        }
    }

    /// Returns the global named `name` in this module.
    /// Returns an error if the global doesn't exist.
    pub fn global<'frame, N, F>(self, frame: &mut F, name: N) -> JlrsResult<Value<'frame, 'static>>
    where
        N: TemporarySymbol,
        F: Frame<'frame>,
    {
        unsafe {
            let symbol = name.temporary_symbol(Private);

            let func = jl_get_global(self.unwrap(Private), symbol.unwrap(Private));
            if func.is_null() {
                Err(JlrsError::GlobalNotFound {
                    name: symbol.as_str().unwrap_or("<Non-UTF8 symbol>").into(),
                    module: self.name().as_str().unwrap_or("<Non-UTF8 symbol>").into(),
                })?;
            }

            frame
                .push_root(NonNull::new_unchecked(func), Private)
                .map_err(Into::into)
        }
    }

    /// Returns the global named `name` in this module.
    /// Returns an error if the global doesn't exist.
    pub fn global_ref<N>(self, name: N) -> JlrsResult<ValueRef<'scope, 'static>>
    where
        N: TemporarySymbol,
    {
        unsafe {
            let symbol = name.temporary_symbol(Private);

            let global = jl_get_global(self.unwrap(Private), symbol.unwrap(Private));
            if global.is_null() {
                return Err(JlrsError::GlobalNotFound {
                    name: symbol.as_str().unwrap_or("<Non-UTF8 symbol>").into(),
                    module: self.name().as_str().unwrap_or("<Non-UTF8 symbol>").into(),
                }
                .into());
            }

            Ok(ValueRef::wrap(global))
        }
    }

    /// Returns the global named `name` in this module as a [`LeakedValue`].
    /// Returns an error if the global doesn't exist.
    pub fn leaked_global<N>(self, name: N) -> JlrsResult<LeakedValue>
    where
        N: TemporarySymbol,
    {
        unsafe {
            let symbol = name.temporary_symbol(Private);

            // there doesn't seem to be a way to check if this is actually a
            // function...
            let func = jl_get_global(self.unwrap(Private), symbol.unwrap(Private));
            if func.is_null() {
                return Err(JlrsError::FunctionNotFound(
                    symbol.as_str().unwrap_or("<Non-UTF8 symbol>").into(),
                )
                .into());
            }

            Ok(LeakedValue::wrap(func))
        }
    }

    /// Returns the function named `name` in this module.
    /// Returns an error if the function doesn't exist or if it's not a subtype of `Core.Function`.
    pub fn function<'frame, N, F>(
        self,
        frame: &mut F,
        name: N,
    ) -> JlrsResult<Function<'frame, 'static>>
    where
        N: TemporarySymbol,
        F: Frame<'frame>,
    {
        unsafe {
            let symbol = name.temporary_symbol(Private);
            let func = self.global(frame, symbol)?;

            if !func.is::<Function>() {
                let name = symbol.as_str().unwrap_or("<Non-UTF8 string>").into();
                let ty = func.type_name().unwrap_or("<Non-UTF8 string>").into();
                Err(JlrsError::NotAFunction { name, ty })?;
            }

            Ok(func.cast_unchecked::<Function>())
        }
    }

    /// Returns the function named `name` in this module.
    /// Returns an error if the function doesn't exist or if it's not a subtype of `Core.Function`.
    pub fn function_ref<N>(self, name: N) -> JlrsResult<FunctionRef<'scope, 'static>>
    where
        N: TemporarySymbol,
    {
        unsafe {
            let symbol = name.temporary_symbol(Private);
            let func = self.global_ref(symbol)?.wrapper_unchecked();

            if !func.is::<Function>() {
                let name = symbol.as_str().unwrap_or("<Non-UTF8 string>").into();
                let ty = func.type_name().unwrap_or("<Non-UTF8 string>").into();
                Err(JlrsError::NotAFunction { name, ty })?;
            }

            Ok(FunctionRef::wrap(func.unwrap(Private)))
        }
    }

    /// Returns the function named `name` in this module as a [`LeakedValue`].
    /// Returns an error if the function doesn't exist.
    pub fn leaked_function<N>(self, name: N) -> JlrsResult<LeakedValue>
    where
        N: TemporarySymbol,
    {
        self.leaked_global(name)
    }

    /// Convert `self` to a `LeakedValue`.
    pub fn as_leaked(self) -> LeakedValue {
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
    pub fn require<'target, 'frame, S, F, N>(
        self,
        scope: S,
        module: N,
    ) -> JlrsResult<S::JuliaResult>
    where
        S: Scope<'target, 'frame, 'static, F>,
        F: Frame<'frame>,
        N: TemporarySymbol,
    {
        unsafe {
            Module::wrap(jl_base_module, Private)
                .function_ref("require")
                .unwrap()
                .wrapper_unchecked()
                .call2(
                    scope,
                    self.as_value(),
                    module.temporary_symbol(Private).as_value(),
                )
        }
    }

    /// TODO: require_ref
    /// Load a module by calling `Base.require` and return this module if it has been loaded
    /// successfully. This method can be used to load parts of the standard library like
    /// `LinearAlgebra`. Unlike `Module::require`, this method will panic if the module cannot
    /// be loaded. Note that the loaded module is not made available in the module used to call
    /// this method, you can use `Module::set_global` to do so.
    ///
    /// Note that when you want to call `using Submodule` in the `Main` module, you can do so by
    /// evaluating the using-statement with [`Value::eval_string`].
    pub fn require_or_panic<S>(self, global: Global<'scope>, module: S) -> Self
    where
        S: TemporarySymbol,
    {
        unsafe {
            Module::base(global)
                .function_ref("require")
                .unwrap()
                .wrapper_unchecked()
                .call2_unrooted(
                    global,
                    self.as_value(),
                    module.temporary_symbol(Private).as_value(),
                )
                .expect(&format!(
                    "Could not load ${:?}",
                    module.temporary_symbol(Private)
                ))
                .wrapper_unchecked()
                .cast_unchecked::<Module>()
        }
    }
}

impl_julia_typecheck!(Module<'frame>, jl_module_type, 'frame);

impl_valid_layout!(Module<'frame>, 'frame);

impl<'frame, 'data> Debug for Module<'frame> {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        let name = self.name().as_str().unwrap_or("<Non-UTF8 symbol>");
        f.debug_tuple("Module").field(&name).finish()
    }
}

impl<'scope> Wrapper<'scope, '_> for Module<'scope> {
    type Internal = jl_module_t;

    unsafe fn wrap_non_null(inner: NonNull<Self::Internal>, _: Private) -> Self {
        Self(inner, PhantomData)
    }

    unsafe fn unwrap_non_null(self, _: Private) -> NonNull<Self::Internal> {
        self.0
    }
}
