# jlrs

[![Rust Docs](https://docs.rs/jlrs/badge.svg)](https://docs.rs/jlrs)
[![License:MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)


jlrs is a crate that provides access to most of the Julia C API, it can be used to embed Julia in Rust applications and to use functionality from the Julia C API when writing `ccall`able functions in Rust. Currently this crate is only tested on Linux in combination with Julia 1.7 and is not compatible with earlier versions of Julia.


## Features

An incomplete list of features that are currently supported by jlrs:

 - Access arbitrary Julia modules and their contents.
 - Call arbitrary Julia functions, including functions that take keyword arguments.
 - Include and call your own Julia code.
 - Load a custom system image.
 - Create values that Julia can use, and convert them back to Rust, from Rust.
 - Access the type information and fields of values and check their properties.
 - Create and use n-dimensional arrays.
 - Support for mapping Julia structs to Rust structs which can be generated with `JlrsReflect.jl`.
 - Structs that can be mapped to Rust include those with type parameters and bits unions.
 - Use these features when calling Rust from Julia through `ccall`.
 - Offload long-running functions to another thread and `.await` the result with the async runtime.


## Usage

Add this to your `Cargo.toml`:

```toml
[dependencies]
jlrs = "0.12"
```

This crate depends on jl-sys which contains the raw bindings to the Julia C API, these are generated by bindgen. You can find the requirements for using bindgen in [their User Guide](https://rust-lang.github.io/rust-bindgen/requirements.html).

#### Linux

The recommended way to install Julia is to download the binaries from the official website, which is distributed in an archive containing a directory called `julia-x.y.z`. This directory contains several other directories, including a `bin` directory containing the `julia` executable.

In order to ensure the `julia.h` header file can be found, either `/usr/include/julia/julia.h` must exist, or you have to set the `JULIA_DIR` environment variable to `/path/to/julia-x.y.z`. This environment variable can be used to override the default. Similarly, in order to load `libjulia.so` you must add `/path/to/julia-x.y.z/lib` to the `LD_LIBRARY_PATH` environment variable.

#### Windows

Support for Windows was dropped in jlrs 0.10 due to compilation and dependency issues. If you  want to use jlrs on Windows you must use WSL. An installation guide to install WSL on Windows can be found [on Microsoft's website](https://docs.microsoft.com/en-us/windows/wsl/install-win10). After installing a Linux distribution, follow the installation instructions for Linux.


## Using this crate

The first thing you should do is `use` the `prelude`-module with an asterisk, this will bring all the structs and traits you're likely to need into scope. If you're calling Julia from Rust, Julia must be initialized before you can use it. You can do so by calling `Julia::init`, which provides you with an instance of `Julia`. Note that this method can only be called once, if you drop its result you won't be able to create a new instance but have to restart the application. If you want to use a custom system image, you must call `Julia::init_with_image` instead of `Julia::init`. If you're calling Rust from Julia everything has already been initialized, you can use `CCall` instead.


### Calling Julia from Rust

With the `Julia` instance it's possible to include files containing Julia code by calling `Julia::include`. In order to actually create values that Julia can use and call Julia functions, `Julia::scope` must be used. This method takes a closure that takes two arguments, a `Global` and a mutable reference to a `GcFrame`. A `Global` is used to access Julia modules, the `GcFrame` is used to ensure Julia data isn't freed by the garbage collector while references to that data can be used from Rust.

Because you can use both a `Global` and a mutable reference to a `GcFrame` inside the closure, it's possible to access the contents of modules and create new values that can be used by Julia. The methods of `Module` let you access the contents of arbitrary modules, several methods are available to create new values.

The simplest is to call `Value::eval_string`, a method that takes two arguments. The first must implement the `Scope` trait, the second is a string which has to contain valid Julia code. The most important thing to know about the `Scope` trait for now is that it's used by functions that create new values to ensure the result is rooted. Mutable references to `GcFrame`s implement `Scope`, in this case the `Value` that is returned is rooted in that frame, so the result is protected from garbage collection until the frame is dropped when that scope ends.

In practice, `Value::eval_string` is relatively limited. It can be used to evaluate simple function calls like `sqrt(2.0)`, but can't take any arguments. Its most important use-case is importing installed packages by evaluating an `import` or `using` statement. A more interesting method, `Value::new`, can be used with data of any type that implements `IntoJulia`. This trait is implemented by primitive types like `i8` and `char`. Any type that implements `IntoJulia` also implements `Unbox` which is used to extract the contents of a Julia value.

In addition to evaluating raw commands with `Value::eval_string`, it's possible to call anything that implements `Call` as a Julia function. `Value` implements this trait because any Julia value is potentially callable as a function. Functions can be called with any number of positional arguments and be provided with keyword arguments. Both `Value::eval_string` and the trait methods of `Call` are unsafe because it's trivial to write a function like `boom() = unsafe_load(Ptr{Float64}(C_NULL))`, which causes a segfault when it's called, and call it with these methods.

As a simple example, let's convert two numbers to Julia values and add them:

```rust
use jlrs::prelude::*;

fn main() {
    // Initializing Julia is unsafe because it can race with another crate that does 
    // the same. 
    let mut julia = unsafe { Julia::init().unwrap() };

    let res = julia.scope(|global, frame| {
        // Create the two arguments. Note that the first argument, something that
        // implements Scope, is taken by value and mutable references don't implement
        // Copy, so it's necessary to mutably reborrow the frame.
        let i = Value::new(&mut *frame, 2u64)?;
        let j = Value::new(&mut *frame, 1u32)?;

        // The `+` function can be found in the base module.
        let func = Module::base(global).function(&mut *frame, "+")?;

        // Call the function and unbox the result as a `u64`. The result of the function
        // call is a nested `Result`; the outer error doesn't  contain to any Julia
        // data, while the inner error contains the exception if one is thrown. Here we
        // explicitly convert the exception to an error that is compatible with `?`.
        unsafe {
            func.call2(&mut *frame, i, j)?
                .into_jlrs_result()?
                .unbox::<u64>()
        }
    }).unwrap();
    
    assert_eq!(res, 3);
}
```

Many more features are available, including creating and accessing n-dimensional Julia arrays and nesting scopes. To learn how to use them, please see the documentation for the `memory` and `wrappers` modules.


### Calling Rust from Julia

Julia's `ccall` interface can be used to call `extern "C"` functions defined in Rust, for most use cases you shouldn't need jlrs. There are two major ways to use `ccall`, with a pointer to the function or a `(:function, "library")` pair.

A function can be cast to a void pointer and converted to a `Value`:

```rust
use jlrs::prelude::*;
// This function will be provided to Julia as a pointer, so its name can be mangled.
unsafe extern "C" fn call_me(arg: bool) -> isize {
    if arg {
        1
    } else {
        -1
    }
}

fn main() {
    let mut julia = unsafe { Julia::init().unwrap() };
    julia.scope(|global, frame| {
        // Cast the function to a void pointer
        let call_me_val = Value::new(&mut *frame, call_me as *mut std::ffi::c_void)?;

        // Value::eval_string can be used to create new functions.
        let func = Value::eval_string(
            &mut *frame,
            "myfunc(callme::Ptr{Cvoid})::Int = ccall(callme, Int, (Bool,), true)"
        )?.unwrap();

        // Call the function and unbox the result.
        let output = func.call1(&mut *frame, call_me_val)?
            .into_jlrs_result()?
            .cast::<isize>()?;

        assert_eq!(output, 1);
        
        Ok(())
    }).unwrap();
}
```

You can also use functions defined in `dylib` and `cdylib` libraries. In order to create such a library you need to add

```toml
[lib]
crate-type = ["dylib"]
```

or

```toml
[lib]
crate-type = ["cdylib"]
```

respectively to your crate's `Cargo.toml`. Use a `dylib` if you want to use the crate in other Rust crates, but if it's only intended to be called through `ccall` a `cdylib` is the better choice. On Linux, compiling such a crate will be compiled to `lib<crate_name>.so`.

The functions you want to use with `ccall` must be both `extern "C"` functions to ensure the C ABI is used, and annotated with `#[no_mangle]` to prevent name mangling. Julia can find libraries in directories that are either on the default library search path or included by setting the `LD_LIBRARY_PATH` environment variable on Linux. If the compiled library is not directly visible to Julia, you can open it with `Libdl.dlopen` and acquire function pointers with `Libdl.dlsym`. These pointers can be called the same way as the pointer in the previous example.

If the library is visible to Julia you can access it with the library name. If `call_me` is defined in a crate called `foo`, the following should workif the function is annotated with `#[no_mangle]`:

```julia
ccall((:call_me, "libfoo"), Int, (Bool,), false)
```

One important aspect of calling Rust from other languages in general is that panicking across an FFI boundary is undefined behaviour. If you're not sure your code will never panic, wrap it with `std::panic::catch_unwind`.

Most features provided by jlrs including accessing modules, calling functions, and borrowing array data require a `Global` or a frame. You can access these by creating a `CCall` first. Another method provided by `CCall` is `CCall::uv_async_send`, this method can be used in combination with `Base.AsyncCondition`. In particular, it lets you write a `ccall`able function that does its actual work on another thread, return early and `wait` on the async condition, which happens when `CCall::uv_async_send` is called when that work is finished. The advantage of this is that the long-running function will not block the Julia runtime, There's an example available on GitHub that shows how to do this.


### Async runtime

The experimental async runtime runs Julia in a separate thread and allows multiple tasks to run in parallel by offloading functions to a new thread in Julia and waiting for them to complete without blocking the runtime. To use this feature you must enable the `async` feature flag:

```toml
[dependencies]
jlrs = { version = "0.12", features = ["async"] }
```

The struct `AsyncJulia` is exported by the prelude and lets you initialize the runtime in two ways, either as a task or as a thread. The first way should be used if you want to integrate the async runtime into a larger project that uses `async_std`. In order for the runtime to work correctly the `JULIA_NUM_THREADS` environment variable must be set to 3 or more, or `auto`.

In order to call Julia with the async runtime you must implement the either the `AsyncTask` or `GeneratorTask` trait. An `AsyncTask` can be called once, its `run` is similar to the closures that are used in the examples above for the sync runtime; it provides you with a `Global` and an `AsyncGcFrame` which provides mostly the same functionality as `GcFrame`. The `AsyncGcFrame` is required to call `CallAsync::call_async` which calls a Julia function on another thread by using `Base.Threads.@spawn` and returns a `Future`. While awaiting the result the runtime can handle another task. If you don't use `CallAsync::call_async` tasks are executed sequentially.

A `GeneratorTask` is more powerful. It has two methods, `init` and `run`, `init` is called when the `GeneratorTask` is started and can be used to prepare the initial state of the generator. The frame provided to `init` is not dropped after it has completed, which means this initial state can contain Julia data. Whenever a `GeneratorTask` is created, a `GeneratorHandle` is returned. This handle can be used to call the `GeneratorTask` which calls its `run` method once. A `GeneratorHandle` can be reused and used from different threads.

You can find basic examples in [the examples directory of the repo](https://github.com/Taaitaaiger/jlrs/tree/master/examples).


## Testing

The restriction that Julia can be initialized once must be taken into account when running tests that use `jlrs`. The recommended approach is to create a thread-local static `RefCell`:

```rust
use jlrs::prelude::*;
use std::cell::RefCell;
thread_local! {
    pub static JULIA: RefCell<Julia> = {
        let julia = RefCell::new(unsafe { Julia::init().unwrap() });
        julia.borrow_mut().scope(|_global, _frame| {
            /* include everything you need to use */
            Ok(())
        }).unwrap();
        julia
    };
}
```

Tests that use this construct can only use one thread for testing, so you must use `cargo test -- --test-threads=1`, otherwise the code above will panic when a test tries to call `Julia::init` a second time from another thread.

If these tests also involve the async runtime, the `JULIA_NUM_THREADS` environment variable must be set to a value larger than 1.

If you want to run jlrs's tests, both these requirements must be taken into account: `JULIA_NUM_THREADS=3 cargo test -- --test-threads=1`


## Custom types

In order to map a struct in Rust to one in Julia you can derive `JuliaStruct`. This will implement `Cast`, `JuliaType`, `ValidLayout`, and `JuliaTypecheck` for that type. If the struct in Julia has no type parameters and is a bits type you can also derive `IntoJulia`, which lets you use the type in combination with `Value::new`.

You should not implement these structs manually. The `JlrsReflect.jl` package can generate the correct Rust struct for types that have no tuple or union fields with type parameters. The reason for this restriction is that the layout of tuple and union fields can be very different depending on these parameters in a way that can't be nicely expressed in Rust.

These custom types can also be used when you call Rust from Julia with `ccall`.
