use std::{
    cell::RefCell,
    collections::VecDeque,
    future::{Future, poll_fn},
    pin::Pin,
    rc::Rc,
    task::{Poll, Waker},
    time::Duration,
};

use boa_engine::{
    Context, JsNativeError, JsResult, JsValue,
    job::{Job, JobExecutor},
};

type PendingJob<'a> = Pin<Box<dyn Future<Output = JsResult<JsValue>> + 'a>>;

#[derive(Default)]
pub(super) struct SnekJobExecutor {
    queue: RefCell<VecDeque<Job>>,
    waker: RefCell<Option<Waker>>,
}

impl SnekJobExecutor {
    fn wake(&self) {
        if let Some(waker) = self.waker.borrow().as_ref() {
            waker.wake_by_ref();
        }
    }
}

impl JobExecutor for SnekJobExecutor {
    fn enqueue_job(self: Rc<Self>, job: Job, _: &mut Context) {
        let token = match &job {
            Job::TimeoutJob(job) if !job.cancelled() => Some(job.cancellation_token()),
            Job::IntervalJob(job) if !job.cancelled() => Some(job.cancellation_token()),
            _ => None,
        };
        if let Some(token) = token {
            let executor = Rc::downgrade(&self);
            token.push_callback(move |_| {
                if let Some(executor) = executor.upgrade() {
                    executor.wake();
                }
            });
        }
        self.queue.borrow_mut().push_back(job);
        self.wake();
    }

    fn run_jobs(self: Rc<Self>, context: &mut Context) -> JsResult<()> {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|error| JsNativeError::error().with_message(error.to_string()))?;
        runtime.block_on(self.run_jobs_async(&RefCell::new(context)))
    }

    async fn run_jobs_async(self: Rc<Self>, context: &RefCell<&mut Context>) -> JsResult<()> {
        let mut pending: Vec<PendingJob<'_>> = Vec::new();
        let mut finalizers: Vec<PendingJob<'_>> = Vec::new();

        let result = poll_fn(|cx| {
            self.waker.borrow_mut().replace(cx.waker().clone());
            let queued = std::mem::take(&mut *self.queue.borrow_mut());
            for job in queued {
                match job {
                    Job::PromiseJob(job) => {
                        job.call(&mut context.borrow_mut())?;
                    }
                    Job::GenericJob(job) => {
                        job.call(&mut context.borrow_mut())?;
                    }
                    Job::AsyncJob(job) => pending.push(Box::pin(job.call(context))),
                    Job::FinalizationRegistryCleanupJob(job) => {
                        finalizers.push(Box::pin(job.call(context)));
                    }
                    Job::TimeoutJob(job) => {
                        let mut sleep = Box::pin(tokio::time::sleep(Duration::from_millis(
                            job.timeout().as_millis(),
                        )));
                        pending.push(Box::pin(async move {
                            poll_fn(|cx| {
                                if job.cancelled() {
                                    Poll::Ready(())
                                } else {
                                    sleep.as_mut().poll(cx)
                                }
                            })
                            .await;
                            if job.cancelled() {
                                Ok(JsValue::undefined())
                            } else {
                                job.call(&mut context.borrow_mut())
                            }
                        }));
                    }
                    Job::IntervalJob(job) => {
                        pending.push(Box::pin(async move {
                            while !job.cancelled() {
                                let mut sleep = Box::pin(tokio::time::sleep(
                                    Duration::from_millis(job.interval().as_millis()),
                                ));
                                poll_fn(|cx| {
                                    if job.cancelled() {
                                        Poll::Ready(())
                                    } else {
                                        sleep.as_mut().poll(cx)
                                    }
                                })
                                .await;
                                if !job.cancelled() {
                                    job.call(&mut context.borrow_mut())?;
                                }
                            }
                            Ok(JsValue::undefined())
                        }));
                    }
                    _ => {
                        return Poll::Ready(Err(JsNativeError::error()
                            .with_message("unsupported Boa job type")
                            .into()));
                    }
                }
            }
            for jobs in [&mut pending, &mut finalizers] {
                let mut index = 0;
                while index < jobs.len() {
                    match jobs[index].as_mut().poll(cx) {
                        Poll::Ready(result) => {
                            drop(jobs.remove(index));
                            result?;
                            cx.waker().wake_by_ref();
                        }
                        Poll::Pending => index += 1,
                    }
                }
            }
            context.borrow_mut().clear_kept_objects();
            if !self.queue.borrow().is_empty() {
                // An async job may need these JS callbacks to complete. Never await
                // just the async futures while runnable callbacks remain queued.
                cx.waker().wake_by_ref();
                Poll::Pending
            } else if pending.is_empty() {
                // FinalizationRegistry cleanup must not keep the runtime alive.
                Poll::Ready(Ok(()))
            } else {
                // The child process (or another pending future) will wake us.
                Poll::Pending
            }
        })
        .await;

        self.waker.borrow_mut().take();
        self.queue.borrow_mut().clear();

        result
    }
}
