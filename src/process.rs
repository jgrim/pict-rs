use crate::store::Store;
use actix_rt::task::JoinHandle;
use actix_web::web::Bytes;
use std::{
    future::Future,
    pin::Pin,
    process::Stdio,
    task::{Context, Poll},
};
use tokio::{
    io::{AsyncRead, AsyncWriteExt, ReadBuf},
    process::{Child, Command},
    sync::oneshot::{channel, Receiver},
};
use tracing::Instrument;
use tracing::Span;

#[derive(Debug)]
struct StatusError;

pub(crate) struct Process {
    child: Child,
    span: Span,
}

struct DropHandle {
    inner: JoinHandle<()>,
}

pin_project_lite::pin_project! {
    struct ProcessRead<I> {
        #[pin]
        inner: I,
        span: Span,
        err_recv: Receiver<std::io::Error>,
        err_closed: bool,
        handle: DropHandle,
    }
}

impl Process {
    pub(crate) fn run(command: &str, args: &[&str]) -> std::io::Result<Self> {
        Self::spawn(Command::new(command).args(args))
    }

    fn spawn_span(&self) -> Span {
        let span = tracing::info_span!(parent: None, "Spawned command writer",);

        span.follows_from(self.span.clone());

        span
    }

    pub(crate) fn spawn(cmd: &mut Command) -> std::io::Result<Self> {
        let cmd = cmd.stdin(Stdio::piped()).stdout(Stdio::piped());

        let span = tracing::info_span!(
            "Spawning Command",
            command = &tracing::field::debug(&cmd),
            exception.message = &tracing::field::Empty,
            exception.details = &tracing::field::Empty,
        );
        cmd.spawn().map(|child| Process { child, span })
    }

    pub(crate) async fn wait(mut self) -> std::io::Result<()> {
        let status = self.child.wait().await?;
        if !status.success() {
            return Err(std::io::Error::new(std::io::ErrorKind::Other, &StatusError));
        }
        Ok(())
    }

    pub(crate) fn bytes_read(mut self, mut input: Bytes) -> Option<impl AsyncRead + Unpin> {
        let mut stdin = self.child.stdin.take()?;
        let stdout = self.child.stdout.take()?;

        let (tx, rx) = channel::<std::io::Error>();

        let span = self.spawn_span();
        let mut child = self.child;
        let handle = actix_rt::spawn(
            async move {
                if let Err(e) = stdin.write_all_buf(&mut input).await {
                    let _ = tx.send(e);
                    return;
                }
                drop(stdin);

                match child.wait().await {
                    Ok(status) => {
                        if !status.success() {
                            let _ = tx
                                .send(std::io::Error::new(std::io::ErrorKind::Other, &StatusError));
                        }
                    }
                    Err(e) => {
                        let _ = tx.send(e);
                    }
                }
            }
            .instrument(span),
        );

        Some(ProcessRead {
            inner: stdout,
            span: self.span,
            err_recv: rx,
            err_closed: false,
            handle: DropHandle { inner: handle },
        })
    }

    pub(crate) fn read(mut self) -> Option<impl AsyncRead + Unpin> {
        let stdout = self.child.stdout.take()?;

        let (tx, rx) = channel();

        let span = self.spawn_span();
        let mut child = self.child;
        let handle = actix_rt::spawn(
            async move {
                match child.wait().await {
                    Ok(status) => {
                        if !status.success() {
                            let _ = tx
                                .send(std::io::Error::new(std::io::ErrorKind::Other, &StatusError));
                        }
                    }
                    Err(e) => {
                        let _ = tx.send(e);
                    }
                }
            }
            .instrument(span),
        );

        Some(ProcessRead {
            inner: stdout,
            span: self.span,
            err_recv: rx,
            err_closed: false,
            handle: DropHandle { inner: handle },
        })
    }

    pub(crate) fn store_read<S: Store>(
        mut self,
        store: S,
        identifier: S::Identifier,
    ) -> Option<impl AsyncRead + Unpin> {
        let mut stdin = self.child.stdin.take()?;
        let stdout = self.child.stdout.take()?;

        let (tx, rx) = channel();

        let span = self.spawn_span();
        let mut child = self.child;
        let handle = actix_rt::spawn(
            async move {
                if let Err(e) = store.read_into(&identifier, &mut stdin).await {
                    let _ = tx.send(e);
                    return;
                }
                drop(stdin);

                match child.wait().await {
                    Ok(status) => {
                        if !status.success() {
                            let _ = tx
                                .send(std::io::Error::new(std::io::ErrorKind::Other, &StatusError));
                        }
                    }
                    Err(e) => {
                        let _ = tx.send(e);
                    }
                }
            }
            .instrument(span),
        );

        Some(ProcessRead {
            inner: stdout,
            span: self.span,
            err_recv: rx,
            err_closed: false,
            handle: DropHandle { inner: handle },
        })
    }
}

impl<I> AsyncRead for ProcessRead<I>
where
    I: AsyncRead,
{
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        let this = self.as_mut().project();

        let span = this.span;
        let err_recv = this.err_recv;
        let err_closed = this.err_closed;
        let inner = this.inner;

        span.in_scope(|| {
            if !*err_closed {
                if let Poll::Ready(res) = Pin::new(err_recv).poll(cx) {
                    *err_closed = true;
                    if let Ok(err) = res {
                        let display = format!("{}", err);
                        let debug = format!("{:?}", err);
                        span.record("exception.message", display.as_str());
                        span.record("exception.details", debug.as_str());
                        return Poll::Ready(Err(err));
                    }
                }
            }

            if let Poll::Ready(res) = inner.poll_read(cx, buf) {
                if let Err(err) = &res {
                    let display = format!("{}", err);
                    let debug = format!("{:?}", err);
                    span.record("exception.message", display.as_str());
                    span.record("exception.details", debug.as_str());
                }
                return Poll::Ready(res);
            }

            Poll::Pending
        })
    }
}

impl Drop for DropHandle {
    fn drop(&mut self) {
        self.inner.abort();
    }
}

impl std::fmt::Display for StatusError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "Command failed with bad status")
    }
}

impl std::error::Error for StatusError {}
