# jlrs

[![Build Status](https://travis-ci.com/Taaitaaiger/jlrs.svg?branch=master)](https://travis-ci.com/Taaitaaiger/jlrs)
[![Coverage Status](https://coveralls.io/repos/github/Taaitaaiger/jlrs/badge.svg?branch=master)](https://coveralls.io/github/Taaitaaiger/jlrs?branch=master)
[![Rust Docs](https://docs.rs/jlrs/badge.svg)](https://docs.rs/jlrs)
[![License:MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

## jlrs

The main goal behind `jlrs` is to provide a simple and safe interface to the Julia C API.
Currently this crate has only been tested on Linux, if you try to use it on another OS it will
likely fail to generate the bindings to Julia. This crate is currently tested with Julia 
v1.4.1.

## Usage

Add this to your `Cargo.toml`:

```toml
[dependencies]
jlrs = "0.3"
```

This crate depends on `jl-sys` which contains the raw bindings to the Julia C API, these are
generated by `bindgen`. The recommended way to install Julia is to download the binaries from
the official website, which is distributed in an archive containing a directory called
`julia-x.y.z`. This directory contains several other directories, including a `bin` directory
containing the `julia` executable.

In order to ensure the `julia.h` header file can be found, you have to set the `JL_PATH`
environment variable to `/path/to/julia-x.y.z`. Similarly, in order to load `libjulia.so` you
must add `/path/to/julia-x.y.z/lib` to the `LD_LIBRARY_PATH` environment variable. If they can
be found at the standard locations, e.g. because you've installed Julia through your package
manager, this is not necessary and things should build without setting the `JL_PATH`
environment variable.

If you create a dynamic library with this crate, the proper symbols must be loaded when your
library is loaded. This can be handled by setting the `RTLD_GLOBAL` flag when loading your
library or by setting `LD_PRELOAD=/path/to/julia-x.y.z/lib/libjulia.so`.

## Features

A small and incomplete list of features that `jlrs` supports: 

 - Call arbitrary functions from the Julia standard library.
 - Include and call your own Julia code.
 - Convert numbers, strings, n-dimensional arrays and more from Rust to Julia and back.
 - Safely borrow array data from both Rust and Julia.
 - Multidimensional indexing of array data with tuples.
 - Access struct fields by name or number.

## Interacting with Julia

The first thing you should do is `use` the `prelude`-module with an asterisk, this will
bring all the structs and traits you're likely to need into scope. Before you can use Julia it
must first be initialized. You do this by calling `Julia::init`. Note that this method can
only be called once, if you drop `Julia` you won't be able to create a new one and have to
restart the entire program.

You can call `Julia::include` to include your own Julia code and either `Julia::frame` or
`Julia::dynamic_frame` to interact with Julia. If you want to create arrays with more than
three dimensions, borrow arrays with more than one, or have improved support for backtraces, 
`jlrs.jl` must be included. You can find this file in the root of this crate's github 
repository. This is necessary because this functionality currently depends on some Julia code 
defined in that file.

The other two methods, `Julia::frame` and `Julia::dynamic_frame`, take a closure that
provides you with a `Global`, and either a `StaticFrame` or `DynamicFrame` respectively. 
`Global` is a token that lets you access Julia modules and their contents, while the frames 
are used to deal with local Julia data. 

Local data must be handled properly: Julia is a programming language with a garbage collector 
that is unaware of any references to data outside of Julia. In order to make it aware of this
usage a stack must be maintained. You choose this stack's size when calling `Julia::init`. 
The elements of this stack are called stack frames; they contain a pointer to the previous 
frame, the number of protected values, and that number of pointers to values. The two frame 
types offered by `jlrs` take care of all the technical details, a `DynamicFrame` will grow 
to the required size while a `StaticFrame` has a definite number of slots. These frames can 
be nested (ie stacked) arbitrarily. 

In order to call a Julia function, you'll need two things: a function to call, and arguments
to call it with. You can acquire the function through the module that defines it with
`Module::function`; `Module::base` and `Module::core` provide access to Julia's `Base`
and `Core` module respectively, while everything you include through `Julia::include` is
made available relative to the `Main` module which you can access by calling `Module::main`.
Most Julia data is represented by a `Value`. Basic data types like numbers, booleans, and
strings can be created through `Value::new` and several methods exist to create an
n-dimensional array. Each value will be protected by a frame, and the two share a lifetime in
order to enforce that a value can be used as long as its protecting frame hasn't been dropped.
Julia functions, their arguments and their results are all `Value`s too. All `Value`s can be 
called as functions, whether this will succeed depends on the value actually being a function.
You can copy data from Julia to Rust by calling `Value::try_unbox`.
As a simple example, let's create two values and add them:

```rust
use jlrs::prelude::*;
fn main() {
    let mut julia = unsafe { Julia::init(16).unwrap() };
    julia.dynamic_frame(|global, frame| {
        // Create the two arguments
        let i = Value::new(frame, 2u64)?;
        let j = Value::new(frame, 1u32)?;
        // We can find the addition-function in the base module
        let func = Module::base(global).function("+")?;
        // Call the function and unbox the result
        let output = func.call2(frame, i, j)?.unwrap();
        output.try_unbox::<u64>()
    }).unwrap();
}
```

You can also do this with a static frame:

```rust
use jlrs::prelude::*;
fn main() {
    let mut julia = unsafe { Julia::init(16).unwrap() };
    // Three slots; two for the inputs and one for the output.
    julia.frame(3, |global, frame| {
        // Create the two arguments, each value requires one slot
        let i = Value::new(frame, 2u64)?;
        let j = Value::new(frame, 1u32)?;
        // We can find the addition-function in the base module
        let func = Module::base(global).function("+")?;
        // Call the function and unbox the result.  
        let output = func.call2(frame, i, j)?.unwrap();
        output.try_unbox::<u64>()
    }).unwrap();
}
```

This is only a small example, other things can be done with `Value` as well: their fields 
can be accessed if the `Value` is some tuple or struct, array data can be borrowed mutably 
or immutably (although only a single array can currently be mutably borrowed at a time). 
Additionally, you can create `Output`s in a frame in order to protect a value from with a 
specific frame; this value will naturally share that frame's lifetime.

For more examples, you can take a look at this crate's integration tests.
