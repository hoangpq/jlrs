use std::{
    ffi::c_void,
    marker::PhantomData,
    pin::Pin,
    ptr::NonNull,
    sync::{Arc, Mutex},
};

use futures::{
    task::{Context, Poll, Waker},
    Future,
};
use jl_sys::{jl_call1, jl_exception_occurred};
use smallvec::SmallVec;

use crate::{
    call::{Call, ProvideKeywords, WithKeywords},
    error::{JuliaResult, CANNOT_DISPLAY_VALUE},
    memory::target::{frame::AsyncGcFrame, unrooted::Unrooted},
    private::Private,
    wrappers::ptr::{
        module::Module,
        private::WrapperPriv,
        task::Task,
        value::{Value, MAX_SIZE},
        Wrapper,
    },
};

pub(crate) struct TaskState<'frame, 'data> {
    completed: bool,
    waker: Option<Waker>,
    task: Option<Task<'frame>>,
    _marker: PhantomData<&'data ()>,
}

pub(crate) struct JuliaFuture<'frame, 'data> {
    shared_state: Arc<Mutex<TaskState<'frame, 'data>>>,
}

impl<'frame, 'data> JuliaFuture<'frame, 'data> {
    pub(crate) fn new<'value, V>(frame: &mut AsyncGcFrame<'frame>, func: Value, values: V) -> Self
    where
        V: AsRef<[Value<'value, 'data>]>,
    {
        let shared_state = Arc::new(Mutex::new(TaskState {
            completed: false,
            waker: None,
            task: None,
            _marker: PhantomData,
        }));

        let values = values.as_ref();
        let state_ptr = Arc::into_raw(shared_state.clone()) as *mut c_void;
        let state_ptr_boxed = Value::new(&mut *frame, state_ptr);

        let mut vals: SmallVec<[Value; MAX_SIZE]> = SmallVec::with_capacity(2 + values.len());

        vals.push(func);
        vals.push(state_ptr_boxed);
        vals.extend_from_slice(values);

        // Safety: module contents are globally rooted, and the function is guaranteed to be safe
        // by the caller.
        let task = unsafe {
            Module::main(&frame)
                .submodule(&frame, "JlrsMultitask")
                .expect("JlrsMultitask not available")
                .wrapper()
                .function(&frame, "asynccall")
                .expect("asynccall not available")
                .wrapper()
                .call(&mut *frame, &mut vals)
                .unwrap_or_else(|e| {
                    let msg = e.display_string_or(CANNOT_DISPLAY_VALUE);
                    panic!("asynccall threw an exception: {}", msg)
                })
                .cast_unchecked::<Task>()
        };

        {
            let locked = shared_state.lock();
            match locked {
                Ok(mut data) => data.task = Some(task),
                _ => panic!("Lock poisoned"),
            }
        }

        JuliaFuture { shared_state }
    }

    #[cfg(any(feature = "nightly", feature = "beta"))]
    pub(crate) fn new_interactive<'value, V>(
        frame: &mut AsyncGcFrame<'frame>,
        func: Value,
        values: V,
    ) -> Self
    where
        V: AsRef<[Value<'value, 'data>]>,
    {
        let shared_state = Arc::new(Mutex::new(TaskState {
            completed: false,
            waker: None,
            task: None,
            _marker: PhantomData,
        }));

        let values = values.as_ref();
        let state_ptr = Arc::into_raw(shared_state.clone()) as *mut c_void;
        let state_ptr_boxed = Value::new(&mut *frame, state_ptr);

        let mut vals: SmallVec<[Value; MAX_SIZE]> = SmallVec::with_capacity(2 + values.len());

        vals.push(func);
        vals.push(state_ptr_boxed);
        vals.extend_from_slice(values);

        // Safety: module contents are globally rooted, and the function is guaranteed to be safe
        // by the caller.
        let task = unsafe {
            Module::main(&frame)
                .submodule(&frame, "JlrsMultitask")
                .expect("JlrsMultitask not available")
                .wrapper()
                .function(&frame, "interactivecall")
                .expect("interactivecall not available")
                .wrapper()
                .call(&mut *frame, &mut vals)
                .unwrap_or_else(|e| {
                    let msg = e.display_string_or(CANNOT_DISPLAY_VALUE);
                    panic!("interactivecall threw an exception: {}", msg)
                })
                .cast_unchecked::<Task>()
        };

        {
            let locked = shared_state.lock();
            match locked {
                Ok(mut data) => data.task = Some(task),
                _ => panic!("Lock poisoned"),
            }
        }

        JuliaFuture { shared_state }
    }

    pub(crate) fn new_local<'value, V>(
        frame: &mut AsyncGcFrame<'frame>,
        func: Value,
        values: V,
    ) -> Self
    where
        V: AsRef<[Value<'value, 'data>]>,
    {
        let shared_state = Arc::new(Mutex::new(TaskState {
            completed: false,
            waker: None,
            task: None,
            _marker: PhantomData,
        }));

        let values = values.as_ref();
        let state_ptr = Arc::into_raw(shared_state.clone()) as *mut c_void;
        let state_ptr_boxed = Value::new(&mut *frame, state_ptr);

        let mut vals: SmallVec<[Value; MAX_SIZE]> = SmallVec::with_capacity(2 + values.len());

        vals.push(func);
        vals.push(state_ptr_boxed);
        vals.extend_from_slice(values);

        // Safety: module contents are globally rooted, and the function is guaranteed to be safe
        // by the caller.
        let task = unsafe {
            Module::main(&frame)
                .submodule(&frame, "JlrsMultitask")
                .expect("JlrsMultitask not available")
                .wrapper()
                .function(&frame, "scheduleasynclocal")
                .expect("scheduleasynclocal not available")
                .wrapper()
                .call(&mut *frame, &mut vals)
                .unwrap_or_else(|e| {
                    let msg = e.display_string_or(CANNOT_DISPLAY_VALUE);
                    panic!("scheduleasynclocal threw an exception: {}", msg)
                })
                .cast_unchecked::<Task>()
        };

        {
            let locked = shared_state.lock();
            match locked {
                Ok(mut data) => data.task = Some(task),
                _ => panic!("Lock poisoned"),
            }
        }

        JuliaFuture { shared_state }
    }

    pub(crate) fn new_main<'value, V>(
        frame: &mut AsyncGcFrame<'frame>,
        func: Value,
        values: V,
    ) -> Self
    where
        V: AsRef<[Value<'value, 'data>]>,
    {
        let shared_state = Arc::new(Mutex::new(TaskState {
            completed: false,
            waker: None,
            task: None,
            _marker: PhantomData,
        }));

        let values = values.as_ref();
        let state_ptr = Arc::into_raw(shared_state.clone()) as *mut c_void;
        let state_ptr_boxed = Value::new(&mut *frame, state_ptr);

        let mut vals: SmallVec<[Value; MAX_SIZE]> = SmallVec::with_capacity(2 + values.len());

        vals.push(func);
        vals.push(state_ptr_boxed);
        vals.extend_from_slice(values);

        // Safety: module contents are globally rooted, and the function is guaranteed to be safe
        // by the caller.
        let task = unsafe {
            Module::main(&frame)
                .submodule(&frame, "JlrsMultitask")
                .expect("JlrsMultitask not available")
                .wrapper()
                .function(&frame, "scheduleasync")
                .expect("scheduleasync not available")
                .wrapper()
                .call(&mut *frame, &mut vals)
                .unwrap_or_else(|e| {
                    let msg = e.display_string_or(CANNOT_DISPLAY_VALUE);
                    panic!("scheduleasync threw an exception: {}", msg)
                })
                .cast_unchecked::<Task>()
        };

        {
            let locked = shared_state.lock();
            match locked {
                Ok(mut data) => data.task = Some(task),
                _ => panic!("Lock poisoned"),
            }
        }

        JuliaFuture { shared_state }
    }

    pub(crate) fn new_posted(
        frame: &mut AsyncGcFrame<'frame>,
        fn_ptr: Value<'_, '_>,
        task_ptr: Value<'_, '_>,
    ) -> Self {
        let shared_state = Arc::new(Mutex::new(TaskState {
            completed: false,
            waker: None,
            task: None,
            _marker: PhantomData,
        }));

        let state_ptr = Arc::into_raw(shared_state.clone()) as *mut c_void;
        let state_ptr_boxed = Value::new(&mut *frame, state_ptr);

        // Safety: module contents are globally rooted, and the function is guaranteed to be safe
        // by the caller.
        let task = unsafe {
            Module::main(&frame)
                .submodule(&frame, "JlrsMultitask")
                .expect("JlrsMultitask not available")
                .wrapper()
                .function(&frame, "postblocking")
                .expect("postblocking not available")
                .wrapper()
                .call3(&mut *frame, fn_ptr, task_ptr, state_ptr_boxed)
                .unwrap_or_else(|e| {
                    let msg = e.display_string_or(CANNOT_DISPLAY_VALUE);
                    panic!("postblocking threw an exception: {}", msg)
                })
                .cast_unchecked::<Task>()
        };

        {
            let locked = shared_state.lock();
            match locked {
                Ok(mut data) => data.task = Some(task),
                _ => panic!("Lock poisoned"),
            }
        }

        JuliaFuture { shared_state }
    }

    pub(crate) fn new_with_keywords<'value, V>(
        frame: &mut AsyncGcFrame<'frame>,
        func: WithKeywords,
        values: V,
    ) -> Self
    where
        V: AsRef<[Value<'value, 'data>]>,
    {
        let shared_state = Arc::new(Mutex::new(TaskState {
            completed: false,
            waker: None,
            task: None,
            _marker: PhantomData,
        }));

        let values = values.as_ref();
        let state_ptr = Arc::into_raw(shared_state.clone()) as *mut c_void;
        let state_ptr_boxed = Value::new(&mut *frame, state_ptr);

        let mut vals: SmallVec<[Value; MAX_SIZE]> = SmallVec::with_capacity(2 + values.len());
        vals.push(func.function());
        vals.push(state_ptr_boxed);
        vals.extend_from_slice(values);

        // Safety: module contents are globally rooted, and the function is guaranteed to be safe
        // by the caller.
        let task = unsafe {
            Module::main(&frame)
                .submodule(&frame, "JlrsMultitask")
                .expect("JlrsMultitask not available")
                .wrapper()
                .function(&frame, "asynccall")
                .expect("asynccall not available")
                .wrapper()
                .provide_keywords(func.keywords())
                .expect("Keywords invalid")
                .call(&mut *frame, &mut vals)
                .unwrap_or_else(|e| {
                    let msg = e.display_string_or(CANNOT_DISPLAY_VALUE);
                    panic!("asynccall threw an exception: {}", msg)
                })
                .cast_unchecked::<Task>()
        };

        {
            let locked = shared_state.lock();
            match locked {
                Ok(mut data) => data.task = Some(task),
                _ => panic!("Lock poisoned"),
            }
        }

        JuliaFuture { shared_state }
    }

    #[cfg(any(feature = "nightly", feature = "beta"))]
    pub(crate) fn new_interactive_with_keywords<'value, V>(
        frame: &mut AsyncGcFrame<'frame>,
        func: WithKeywords,
        values: V,
    ) -> Self
    where
        V: AsRef<[Value<'value, 'data>]>,
    {
        let shared_state = Arc::new(Mutex::new(TaskState {
            completed: false,
            waker: None,
            task: None,
            _marker: PhantomData,
        }));

        let values = values.as_ref();
        let state_ptr = Arc::into_raw(shared_state.clone()) as *mut c_void;
        let state_ptr_boxed = Value::new(&mut *frame, state_ptr);

        let mut vals: SmallVec<[Value; MAX_SIZE]> = SmallVec::with_capacity(2 + values.len());
        vals.push(func.function());
        vals.push(state_ptr_boxed);
        vals.extend_from_slice(values);

        // Safety: module contents are globally rooted, and the function is guaranteed to be safe
        // by the caller.
        let task = unsafe {
            Module::main(&frame)
                .submodule(&frame, "JlrsMultitask")
                .expect("JlrsMultitask not available")
                .wrapper()
                .function(&frame, "interactivecall")
                .expect("interactivecall not available")
                .wrapper()
                .provide_keywords(func.keywords())
                .expect("Keywords invalid")
                .call(&mut *frame, &mut vals)
                .unwrap_or_else(|e| {
                    let msg = e.display_string_or(CANNOT_DISPLAY_VALUE);
                    panic!("interactivecall threw an exception: {}", msg)
                })
                .cast_unchecked::<Task>()
        };

        {
            let locked = shared_state.lock();
            match locked {
                Ok(mut data) => data.task = Some(task),
                _ => panic!("Lock poisoned"),
            }
        }

        JuliaFuture { shared_state }
    }

    pub(crate) fn new_local_with_keywords<'value, V>(
        frame: &mut AsyncGcFrame<'frame>,
        func: WithKeywords,
        values: V,
    ) -> Self
    where
        V: AsRef<[Value<'value, 'data>]>,
    {
        let shared_state = Arc::new(Mutex::new(TaskState {
            completed: false,
            waker: None,
            task: None,
            _marker: PhantomData,
        }));

        let values = values.as_ref();
        let state_ptr = Arc::into_raw(shared_state.clone()) as *mut c_void;
        let state_ptr_boxed = Value::new(&mut *frame, state_ptr);

        let mut vals: SmallVec<[Value; MAX_SIZE]> = SmallVec::with_capacity(2 + values.len());

        vals.push(func.function());
        vals.push(state_ptr_boxed);
        vals.extend_from_slice(values);

        // Safety: module contents are globally rooted, and the function is guaranteed to be safe
        // by the caller.
        unsafe {
            let task = Module::main(&frame)
                .submodule(&frame, "JlrsMultitask")
                .expect("JlrsMultitask not available")
                .wrapper()
                .function(&frame, "scheduleasynclocal")
                .expect("scheduleasynclocal not available")
                .wrapper()
                .provide_keywords(func.keywords())
                .expect("Keywords invalid")
                .call(&mut *frame, &mut vals)
                .unwrap_or_else(|e| {
                    let msg = e.display_string_or(CANNOT_DISPLAY_VALUE);
                    panic!("scheduleasynclocal threw an exception: {}", msg)
                })
                .cast_unchecked::<Task>();

            {
                let locked = shared_state.lock();
                match locked {
                    Ok(mut data) => data.task = Some(task),
                    _ => panic!("Lock poisoned"),
                }
            }

            JuliaFuture { shared_state }
        }
    }

    pub(crate) fn new_main_with_keywords<'value, V>(
        frame: &mut AsyncGcFrame<'frame>,
        func: WithKeywords,
        values: V,
    ) -> Self
    where
        V: AsRef<[Value<'value, 'data>]>,
    {
        let shared_state = Arc::new(Mutex::new(TaskState {
            completed: false,
            waker: None,
            task: None,
            _marker: PhantomData,
        }));

        let values = values.as_ref();
        let state_ptr = Arc::into_raw(shared_state.clone()) as *mut c_void;
        let state_ptr_boxed = Value::new(&mut *frame, state_ptr);

        let mut vals: SmallVec<[Value; MAX_SIZE]> = SmallVec::with_capacity(2 + values.len());

        vals.push(func.function());
        vals.push(state_ptr_boxed);
        vals.extend_from_slice(values);

        // Safety: module contents are globally rooted, and the function is guaranteed to be safe
        // by the caller.
        let task = unsafe {
            Module::main(&frame)
                .submodule(&frame, "JlrsMultitask")
                .expect("JlrsMultitask not available")
                .wrapper()
                .function(&frame, "scheduleasync")
                .expect("scheduleasync not available")
                .wrapper()
                .provide_keywords(func.keywords())
                .expect("Keywords invalid")
                .call(&mut *frame, &mut vals)
                .unwrap_or_else(|e| {
                    let msg = e.display_string_or(CANNOT_DISPLAY_VALUE);
                    panic!("asynccall threw an exception: {}", msg)
                })
                .cast_unchecked::<Task>()
        };

        {
            let locked = shared_state.lock();
            match locked {
                Ok(mut data) => data.task = Some(task),
                _ => panic!("Lock poisoned"),
            }
        }

        JuliaFuture { shared_state }
    }
}

impl<'frame, 'data> Future for JuliaFuture<'frame, 'data> {
    type Output = JuliaResult<'frame, 'data>;
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut shared_state = self.shared_state.lock().unwrap();
        if shared_state.completed {
            if let Some(task) = shared_state.task {
                // Safety: module contents are globally rooted, and fetch is safe to call. The
                // result is reachable through the task which must be rooted at ths point.
                unsafe {
                    let global = Unrooted::new();
                    let f = Module::base(&global)
                        .function(&global, "fetch")
                        .unwrap()
                        .wrapper();

                    let res = jl_call1(f.unwrap(Private), task.unwrap(Private).cast());
                    let exc = jl_exception_occurred();

                    if exc.is_null() {
                        Poll::Ready(Ok(Value::wrap_non_null(
                            NonNull::new_unchecked(res),
                            Private,
                        )))
                    } else {
                        Poll::Ready(Err(Value::wrap_non_null(
                            NonNull::new_unchecked(exc),
                            Private,
                        )))
                    }
                }
            } else {
                // JuliaFuture is not created if task cannot be set
                unreachable!()
            }
        } else if shared_state.waker.is_none() {
            shared_state.waker = Some(cx.waker().clone());
            Poll::Pending
        } else {
            Poll::Pending
        }
    }
}

// This function is called using `ccall` to indicate a task has completed.
#[cfg(feature = "async-rt")]
pub(crate) unsafe extern "C" fn wake_task(state: *const Mutex<TaskState>) {
    let state = Arc::from_raw(state);
    let shared_state = state.lock();
    match shared_state {
        Ok(mut state) => {
            state.completed = true;
            state.waker.take().map(|waker| waker.wake());
        }
        Err(_) => (),
    }
}
