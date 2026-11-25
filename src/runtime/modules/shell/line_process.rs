use std::{
    cell::RefCell,
    future::{Future, poll_fn},
    pin::pin,
    process::Stdio,
    task::Poll,
};

use tokio::{
    io::{AsyncBufReadExt, BufReader},
    process::Child,
    sync::{Mutex, mpsc, watch},
};

use super::{ShellEnvironment, environment::PreparedCommand};

#[derive(Clone)]
enum Failure {
    Cancelled,
    Process(String),
    Cleanup(String),
}

impl Failure {
    fn message(self) -> String {
        match self {
            Self::Cancelled => "shell command was cancelled".into(),
            Self::Process(message) | Self::Cleanup(message) => message,
        }
    }
}

type Outcome = Result<(), Failure>;
type Status = Option<Outcome>;

/// One bounded line queue and one completion signal, shared by the iterator and command.
pub(super) struct LineProcess {
    pending: RefCell<Option<Pending>>,
    reader: Mutex<Reader>,
    cancel: watch::Sender<bool>,
    status: watch::Receiver<Status>,
}

struct Pending {
    words: Vec<String>,
    environment: ShellEnvironment,
    sender: mpsc::Sender<Vec<u8>>,
    status: watch::Sender<Status>,
}

struct Reader {
    receiver: mpsc::Receiver<Vec<u8>>,
    done: bool,
}

impl LineProcess {
    pub(super) fn new(words: Vec<String>, environment: ShellEnvironment) -> Self {
        let (sender, receiver) = mpsc::channel(1);
        let (cancel, _) = watch::channel(false);
        let (status_sender, status) = watch::channel(None);
        Self {
            pending: RefCell::new(Some(Pending {
                words,
                environment,
                sender,
                status: status_sender,
            })),
            reader: Mutex::new(Reader {
                receiver,
                done: false,
            }),
            cancel,
            status,
        }
    }

    /// Start only inside a Tokio task; unused iterators never spawn a child.
    fn start(&self) {
        let Some(pending) = self.pending.borrow_mut().take() else {
            return;
        };
        if *self.cancel.borrow() {
            pending.status.send_replace(Some(Err(Failure::Cancelled)));
            return;
        }
        let cancel = self.cancel.subscribe();
        tokio::spawn(async move {
            let result = produce(pending.words, pending.environment, &pending.sender, cancel).await;
            // Publish the outcome before closing the line queue.
            pending.status.send_replace(Some(result));
        });
    }

    /// Serialize concurrent next() calls while leaving cancellation independent.
    pub(super) async fn next(&self) -> Result<Option<Vec<u8>>, String> {
        self.start();
        let mut reader = self.reader.lock().await;
        if reader.done {
            return Ok(None);
        }
        if *self.cancel.borrow() {
            reader.done = true;
            return Ok(None);
        }
        if let Some(line) = reader.receiver.recv().await
            && !*self.cancel.borrow()
        {
            return Ok(Some(line));
        }
        reader.done = true;
        if *self.cancel.borrow() {
            return Ok(None);
        }
        self.completion().await?;
        Ok(None)
    }

    /// Signal immediately so return() can interrupt a blocked read or full queue.
    pub(super) fn cancel(&self) {
        self.cancel.send_replace(true);
    }

    pub(super) async fn close(&self) -> Result<(), String> {
        self.start();
        let mut reader = self.reader.lock().await;
        reader.done = true;
        reader.receiver.close();
        while reader.receiver.try_recv().is_ok() {}
        drop(reader);
        // Cancellation is expected here, but still wait for child reaping.
        let outcome = self.outcome().await;
        match outcome {
            Err(Failure::Cleanup(error)) => Err(error),
            _ => Ok(()),
        }
    }

    pub(super) async fn completion(&self) -> Result<(), String> {
        self.outcome().await.map_err(Failure::message)
    }

    async fn outcome(&self) -> Outcome {
        self.start();
        let mut status = self.status.clone();
        loop {
            if let Some(result) = status.borrow().clone() {
                return result;
            }
            status.changed().await.map_err(|_| {
                Failure::Cleanup("shell line task ended without a result".to_owned())
            })?;
        }
    }
}

/// Race I/O against cancellation without holding a JavaScript context borrow.
async fn cancellable<T>(
    cancel: &mut watch::Receiver<bool>,
    work: impl Future<Output = T>,
) -> Option<T> {
    let mut work = pin!(work);
    let mut cancelled = pin!(cancel.wait_for(|value| *value));
    poll_fn(|cx| {
        if cancelled.as_mut().poll(cx).is_ready() {
            return Poll::Ready(None);
        }
        work.as_mut().poll(cx).map(Some)
    })
    .await
}

async fn stop(child: &mut Child, program: &str) -> Outcome {
    if child
        .try_wait()
        .map_err(|error| Failure::Cleanup(format!("could not stop '{program}': {error}")))?
        .is_some()
    {
        return Ok(());
    }
    child
        .kill()
        .await
        .map_err(|error| Failure::Cleanup(format!("could not stop '{program}': {error}")))
}

/// Read at most one queued line ahead; a slow consumer applies pipe backpressure.
async fn produce(
    words: Vec<String>,
    environment: ShellEnvironment,
    sender: &mpsc::Sender<Vec<u8>>,
    mut cancel: watch::Receiver<bool>,
) -> Outcome {
    if *cancel.borrow() {
        return Err(Failure::Cancelled);
    }
    let program = &words[0];
    let mut command = match environment.prepare(&words).map_err(Failure::Process)? {
        PreparedCommand::External(command) => command,
        PreparedCommand::Builtin(0) => return Ok(()),
        PreparedCommand::Builtin(code) => {
            return Err(Failure::Process(format!(
                "'{program}' failed: exit status: {code}"
            )));
        }
    };
    let mut child = command
        .stdin(Stdio::inherit())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .kill_on_drop(true)
        .spawn()
        .map_err(|error| Failure::Process(format!("could not start '{program}': {error}")))?;
    let mut reader = BufReader::new(child.stdout.take().expect("stdout is piped"));
    let mut line = Vec::new();
    let read_error = loop {
        match cancellable(&mut cancel, reader.read_until(b'\n', &mut line)).await {
            None => break None,
            Some(Err(error)) => break Some(format!("could not read '{program}' stdout: {error}")),
            Some(Ok(0)) => {
                drop(reader);
                return match cancellable(&mut cancel, child.wait()).await {
                    Some(Ok(status)) if status.success() => Ok(()),
                    Some(Ok(status)) => {
                        Err(Failure::Process(format!("'{program}' failed: {status}")))
                    }
                    Some(Err(error)) => {
                        stop(&mut child, program).await?;
                        Err(Failure::Process(format!(
                            "could not wait for '{program}': {error}"
                        )))
                    }
                    None => {
                        stop(&mut child, program).await?;
                        Err(Failure::Cancelled)
                    }
                };
            }
            Some(Ok(_)) => {
                if line.last() == Some(&b'\n') {
                    line.pop();
                    if line.last() == Some(&b'\r') {
                        line.pop();
                    }
                }
                if !matches!(
                    cancellable(&mut cancel, sender.send(std::mem::take(&mut line))).await,
                    Some(Ok(()))
                ) {
                    break None;
                }
            }
        }
    };
    drop(reader);
    stop(&mut child, program).await?;
    Err(read_error
        .map(Failure::Process)
        .unwrap_or(Failure::Cancelled))
}
