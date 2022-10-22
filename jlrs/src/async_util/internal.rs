use super::{channel::Channel, task::PersistentTask};
use crate::async_util::channel::ChannelReceiver;
use crate::async_util::channel::OneshotSender;
use crate::call::Call;
use crate::error::JlrsError;
use crate::error::JlrsResult;
use crate::memory::context::stack::Stack;
use crate::memory::target::frame::{AsyncGcFrame, GcFrame};
use crate::prelude::JuliaString;
use crate::prelude::Module;
use crate::prelude::Value;
use crate::prelude::Wrapper;
use crate::runtime::async_rt::PersistentHandle;
use crate::{async_util::task::AsyncTask, runtime::async_rt::PersistentMessage};
use async_trait::async_trait;
use std::path::PathBuf;
use std::{marker::PhantomData, num::NonZeroUsize, sync::Arc};

pub(crate) type InnerPersistentMessage<P> = Box<
    dyn CallPersistentTaskEnvelope<
        Input = <P as PersistentTask>::Input,
        Output = <P as PersistentTask>::Output,
    >,
>;

// What follows is a significant amount of indirection to allow different tasks to have a
// different Output, and allow users to provide an arbitrary sender that implements ReturnChannel
// to return some result.
pub(crate) enum Task {}
pub(crate) enum RegisterTask {}
pub(crate) enum Persistent {}
pub(crate) enum RegisterPersistent {}

pub(crate) struct CallPersistentTask<I, O, S>
where
    I: Send + Sync,
    O: Send + Sync + 'static,
    S: OneshotSender<JlrsResult<O>>,
{
    pub(crate) sender: S,

    pub(crate) input: Option<I>,

    pub(crate) _marker: PhantomData<O>,
}

#[async_trait(?Send)]
trait AsyncTaskEnvelope: Send {
    type A: AsyncTask + Send;

    async fn call_run<'inner>(
        &'inner mut self,
        frame: AsyncGcFrame<'static>,
    ) -> JlrsResult<<Self::A as AsyncTask>::Output>;
}

#[async_trait(?Send)]
impl<A: AsyncTask> AsyncTaskEnvelope for A {
    type A = Self;
    async fn call_run<'inner>(
        &'inner mut self,
        frame: AsyncGcFrame<'static>,
    ) -> JlrsResult<<Self::A as AsyncTask>::Output> {
        self.run(frame).await
    }
}

#[async_trait(?Send)]
trait PersistentTaskEnvelope: Send {
    type P: PersistentTask + Send;

    async fn call_init<'inner>(
        &'inner mut self,
        frame: AsyncGcFrame<'static>,
    ) -> JlrsResult<<Self::P as PersistentTask>::State>;

    async fn call_run<'inner>(
        &'inner mut self,
        frame: AsyncGcFrame<'static>,
        state: &'inner mut <Self::P as PersistentTask>::State,
        input: <Self::P as PersistentTask>::Input,
    ) -> JlrsResult<<Self::P as PersistentTask>::Output>;
}

#[async_trait(?Send)]
impl<P> PersistentTaskEnvelope for P
where
    P: PersistentTask,
{
    type P = Self;

    async fn call_init<'inner>(
        &'inner mut self,
        frame: AsyncGcFrame<'static>,
    ) -> JlrsResult<<Self::P as PersistentTask>::State> {
        {
            self.init(frame).await
        }
    }

    async fn call_run<'inner>(
        &'inner mut self,
        mut frame: AsyncGcFrame<'static>,
        state: &'inner mut <Self::P as PersistentTask>::State,
        input: <Self::P as PersistentTask>::Input,
    ) -> JlrsResult<<Self::P as PersistentTask>::Output> {
        {
            let output = {
                let (owner, nested) = frame.nest_async();
                let res = self.run(nested, state, input).await;
                std::mem::drop(owner);
                res
            };

            output
        }
    }
}

pub(crate) trait CallPersistentTaskEnvelope: Send + Sync {
    type Input;
    type Output;

    fn respond(self: Box<Self>, result: JlrsResult<Self::Output>);
    fn input(&mut self) -> Self::Input;
}

impl<I, O, S> CallPersistentTaskEnvelope for CallPersistentTask<I, O, S>
where
    I: Send + Sync,
    O: Send + Sync,
    S: OneshotSender<JlrsResult<O>>,
{
    type Input = I;
    type Output = O;

    fn respond(self: Box<Self>, result: JlrsResult<Self::Output>) {
        Box::new(self.sender).send(result)
    }

    fn input(&mut self) -> Self::Input {
        self.input.take().unwrap()
    }
}

pub(crate) struct PersistentComms<C, P, O> {
    sender: O,
    _task: PhantomData<P>,
    _channel: PhantomData<C>,
}

impl<C, P, O> PersistentComms<C, P, O>
where
    C: Channel<PersistentMessage<P>>,
    P: PersistentTask,
    O: OneshotSender<JlrsResult<PersistentHandle<P>>>,
{
    pub(crate) fn new(sender: O) -> Self {
        PersistentComms {
            sender,
            _task: PhantomData,
            _channel: PhantomData,
        }
    }
}

impl<C, P, O> PendingTask<PersistentComms<C, P, O>, P, Persistent>
where
    C: Channel<PersistentMessage<P>>,
    P: PersistentTask,
    O: OneshotSender<JlrsResult<PersistentHandle<P>>>,
{
    pub(crate) fn new(task: P, sender: PersistentComms<C, P, O>) -> Self {
        PendingTask {
            task: Some(task),
            sender,
            _kind: PhantomData,
        }
    }

    fn split(self) -> (P, PersistentComms<C, P, O>) {
        (self.task.unwrap(), self.sender)
    }
}

pub(crate) struct PendingTask<O, T, Kind> {
    task: Option<T>,
    sender: O,
    _kind: PhantomData<Kind>,
}

impl<O, A> PendingTask<O, A, Task>
where
    O: OneshotSender<JlrsResult<A::Output>>,
    A: AsyncTask,
{
    pub(crate) fn new(task: A, sender: O) -> Self {
        PendingTask {
            task: Some(task),
            sender,
            _kind: PhantomData,
        }
    }

    fn split(self) -> (A, O) {
        (self.task.unwrap(), self.sender)
    }
}

impl<O, A> PendingTask<O, A, RegisterTask>
where
    O: OneshotSender<JlrsResult<()>>,
    A: AsyncTask,
{
    pub(crate) fn new(sender: O) -> Self {
        PendingTask {
            task: None,
            sender,
            _kind: PhantomData,
        }
    }

    fn sender(self) -> O {
        self.sender
    }
}

impl<O, P> PendingTask<O, P, RegisterPersistent>
where
    O: OneshotSender<JlrsResult<()>>,
    P: PersistentTask,
{
    pub(crate) fn new(sender: O) -> Self {
        PendingTask {
            task: None,
            sender,
            _kind: PhantomData,
        }
    }

    fn sender(self) -> O {
        self.sender
    }
}

#[async_trait(?Send)]
pub(crate) trait PendingTaskEnvelope: Send + Sync {
    async fn call(mut self: Box<Self>, mut stack: &'static Stack);
}

#[async_trait(?Send)]
impl<O, A> PendingTaskEnvelope for PendingTask<O, A, Task>
where
    O: OneshotSender<JlrsResult<A::Output>>,
    A: AsyncTask,
{
    async fn call(mut self: Box<Self>, stack: &'static Stack) {
        let (mut task, result_sender) = self.split();

        // Safety: the stack slots can be reallocated because it doesn't contain any frames
        // yet. The frame is dropped at the end of the scope, the nested hierarchy of scopes is
        // maintained.
        let res = unsafe {
            let (owner, frame) = AsyncGcFrame::base(&stack);

            let res = task.call_run(frame).await;
            std::mem::drop(owner);
            res
        };

        result_sender.send(res);
    }
}

#[async_trait(?Send)]
impl<O, A> PendingTaskEnvelope for PendingTask<O, A, RegisterTask>
where
    O: OneshotSender<JlrsResult<()>>,
    A: AsyncTask,
{
    async fn call(mut self: Box<Self>, stack: &'static Stack) {
        let sender = self.sender();

        // Safety: the stack slots can be reallocated because it doesn't contain any frames
        // yet. The frame is dropped at the end of the scope, the nested hierarchy of scopes is
        // maintained.
        let res = unsafe {
            let (owner, frame) = AsyncGcFrame::base(&stack);
            let res = A::register(frame).await;
            std::mem::drop(owner);
            res
        };

        sender.send(res);
    }
}

#[async_trait(?Send)]
impl<O, P> PendingTaskEnvelope for PendingTask<O, P, RegisterPersistent>
where
    O: OneshotSender<JlrsResult<()>>,
    P: PersistentTask,
{
    async fn call(mut self: Box<Self>, stack: &'static Stack) {
        let sender = self.sender();

        // Safety: the stack slots can be reallocated because it doesn't contain any frames
        // yet. The frame is dropped at the end of the scope, the nested hierarchy of scopes is
        // maintained.
        let res = unsafe {
            let (owner, frame) = AsyncGcFrame::base(&stack);
            let res = P::register(frame).await;
            std::mem::drop(owner);
            res
        };

        sender.send(res);
    }
}

#[async_trait(?Send)]
impl<C, P, O> PendingTaskEnvelope for PendingTask<PersistentComms<C, P, O>, P, Persistent>
where
    C: Channel<PersistentMessage<P>>,
    O: OneshotSender<JlrsResult<PersistentHandle<P>>>,
    P: PersistentTask,
{
    async fn call(mut self: Box<Self>, stack: &'static Stack) {
        let (mut persistent, handle_sender) = self.split();
        let handle_sender = handle_sender.sender;
        let (sender, mut receiver) = C::channel(NonZeroUsize::new(P::CHANNEL_CAPACITY));
        // Safety: the stack slots can be reallocated because it doesn't contain any frames
        // yet. The frame is dropped at the end of the scope, the nested hierarchy of scopes is
        // maintained.
        unsafe {
            let (owner, frame) = AsyncGcFrame::base(&stack);

            match persistent.call_init(frame).await {
                Ok(mut state) => {
                    handle_sender.send(Ok(PersistentHandle::new(Arc::new(sender))));

                    let offset = stack.size();

                    loop {
                        let mut msg = match receiver.recv().await {
                            Ok(msg) => msg.msg,
                            Err(_) => break,
                        };

                        let frame = owner.reconstruct(offset);
                        let res = persistent.call_run(frame, &mut state, msg.input()).await;

                        msg.respond(res);
                    }

                    let frame = owner.reconstruct(offset);
                    persistent.exit(frame, &mut state).await;
                }
                Err(e) => handle_sender.send(Err(e)),
            }

            std::mem::drop(owner);
        }
    }
}

pub(crate) struct BlockingTask<F, O, T> {
    func: F,
    sender: O,
    _res: PhantomData<T>,
}

impl<F, O, T> BlockingTask<F, O, T>
where
    for<'base> F: Send + Sync + FnOnce(GcFrame<'base>) -> JlrsResult<T>,
    O: OneshotSender<JlrsResult<T>>,
    T: Send + Sync + 'static,
{
    pub(crate) fn new(func: F, sender: O) -> Self {
        Self {
            func,
            sender,
            _res: PhantomData,
        }
    }

    fn call<'scope>(self: Box<Self>, frame: GcFrame<'scope>) -> (JlrsResult<T>, O) {
        // Safety: this method is called from a thread known to Julia, the lifetime is limited to
        // 'scope.
        let func = self.func;
        let res = func(frame);
        (res, self.sender)
    }
}

pub(crate) trait BlockingTaskEnvelope: Send + Sync {
    fn call<'scope>(self: Box<Self>, stack: &'scope Stack);
}

impl<F, O, T> BlockingTaskEnvelope for BlockingTask<F, O, T>
where
    for<'base> F: Send + Sync + FnOnce(GcFrame<'base>) -> JlrsResult<T>,
    O: OneshotSender<JlrsResult<T>>,
    T: Send + Sync + 'static,
{
    fn call<'scope>(self: Box<Self>, stack: &'scope Stack) {
        // Safety: the stack slots can be reallocated because it doesn't contain any frames
        // yet. The frame is dropped at the end of the scope, the nested hierarchy of scopes is
        // maintained.
        let (res, ch) = unsafe {
            let (owner, frame) = GcFrame::base(&stack);
            let res = self.call(frame);
            std::mem::drop(owner);
            res
        };

        OneshotSender::send(ch, res);
    }
}

pub(crate) struct IncludeTask<O> {
    path: PathBuf,
    sender: O,
}

impl<O> IncludeTask<O>
where
    O: OneshotSender<JlrsResult<()>>,
{
    pub(crate) fn new(path: PathBuf, sender: O) -> Self {
        Self { path, sender }
    }

    unsafe fn call_inner<'scope>(mut frame: GcFrame<'scope>, path: PathBuf) -> JlrsResult<()> {
        match path.to_str() {
            Some(path) => {
                let path = JuliaString::new(&mut frame, path);
                Module::main(&frame)
                    .function(&frame, "include")?
                    .wrapper_unchecked()
                    .call1(&frame, path.as_value())
                    .map_err(|e| {
                        JlrsError::exception(format!("Include error: {:?}", e.value_unchecked()))
                    })?;
            }
            None => {}
        }

        Ok(())
    }

    fn call<'scope>(self: Box<Self>, frame: GcFrame<'scope>) -> (JlrsResult<()>, O) {
        // Safety: this method is called from a thread known to Julia, the lifetime is limited to
        // 'scope.
        let path = self.path;
        let res = unsafe { Self::call_inner(frame, path) };
        (res, self.sender)
    }
}

pub(crate) trait IncludeTaskEnvelope: Send + Sync {
    fn call(self: Box<Self>, stack: &'static Stack);
}

impl<O> IncludeTaskEnvelope for IncludeTask<O>
where
    O: OneshotSender<JlrsResult<()>>,
{
    fn call(self: Box<Self>, stack: &'static Stack) {
        // Safety: the stack slots can be reallocated because it doesn't contain any frames
        // yet. The frame is dropped at the end of the scope, the nested hierarchy of scopes is
        // maintained.
        let (res, ch) = unsafe {
            let (owner, frame) = GcFrame::base(&stack);
            let res = self.call(frame);
            std::mem::drop(owner);
            res
        };

        OneshotSender::send(ch, res);
    }
}

pub(crate) struct SetErrorColorTask<O> {
    enable: bool,
    sender: O,
}

impl<O> SetErrorColorTask<O>
where
    O: OneshotSender<JlrsResult<()>>,
{
    pub(crate) fn new(enable: bool, sender: O) -> Self {
        Self { enable, sender }
    }

    unsafe fn call_inner<'scope>(frame: GcFrame<'scope>, enable: bool) -> JlrsResult<()> {
        let global = frame.global();

        let enable = if enable {
            Value::true_v(&global)
        } else {
            Value::false_v(&global)
        };

        Module::main(&global)
            .submodule(&global, "Jlrs")?
            .wrapper_unchecked()
            .global(&global, "color")?
            .value_unchecked()
            .set_nth_field_unchecked(0, enable);

        Ok(())
    }

    fn call<'scope>(self: Box<Self>, frame: GcFrame<'scope>) -> (JlrsResult<()>, O) {
        // Safety: this method is called from a thread known to Julia, the lifetime is limited to
        // 'scope.
        let enable = self.enable;
        let res = unsafe { Self::call_inner(frame, enable) };
        (res, self.sender)
    }
}

pub(crate) trait SetErrorColorTaskEnvelope: Send + Sync {
    fn call(self: Box<Self>, stack: &'static Stack);
}

impl<O> SetErrorColorTaskEnvelope for SetErrorColorTask<O>
where
    O: OneshotSender<JlrsResult<()>>,
{
    fn call(self: Box<Self>, stack: &'static Stack) {
        // Safety: the stack slots can be reallocated because it doesn't contain any frames
        // yet. The frame is dropped at the end of the scope, the nested hierarchy of scopes is
        // maintained.
        let (res, ch) = unsafe {
            let (owner, frame) = GcFrame::base(&stack);
            let res = self.call(frame);
            std::mem::drop(owner);
            res
        };

        OneshotSender::send(ch, res);
    }
}
