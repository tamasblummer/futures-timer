//! Extension traits for the standard `Stream` and `Future` traits.

use std::time::{Duration, Instant};
use std::io;

use futures::prelude::*;

use Delay;

/// An extension trait for futures which provides convenient accessors for
/// timing out execution and such.
pub trait FutureExt: Future + Sized {

    /// Creates a new future which will take at most `dur` time to resolve from
    /// the point at which this method is called.
    ///
    /// This combinator creates a new future which wraps the receiving future
    /// in a timeout. The future returned will resolve in at most `dur` time
    /// specified (relative to when this function is called).
    ///
    /// If the future completes before `dur` elapses then the future will
    /// resolve with that item. Otherwise the future will resolve to an error
    /// once `dur` has elapsed.
    ///
    /// # Examples
    ///
    /// ```
    /// extern crate futures;
    /// extern crate futures_timer;
    ///
    /// use std::time::Duration;
    /// use futures::prelude::*;
    /// use futures::executor::block_on;
    /// use futures_timer::FutureExt;
    ///
    /// # fn long_future() -> futures::future::FutureResult<(), std::io::Error> {
    /// #   futures::future::ok(())
    /// # }
    /// #
    /// fn main() {
    ///     let future = long_future();
    ///     let timed_out = future.timeout(Duration::from_secs(1));
    ///
    ///     match block_on(timed_out) {
    ///         Ok(item) => println!("got {:?} within enough time!", item),
    ///         Err(_) => println!("took too long to produce the item"),
    ///     }
    /// }
    /// ```
    fn timeout(self, dur: Duration) -> Timeout<Self>
        where Self::Error: From<io::Error>,
    {
        Timeout {
            timeout: Delay::new(dur),
            future: self,
        }
    }

    /// Creates a new future which will resolve no later than `at` specified.
    ///
    /// This method is otherwise equivalent to the `timeout` method except that
    /// it tweaks the moment at when the timeout elapsed to being specified with
    /// an absolute value rather than a relative one. For more documentation see
    /// the `timeout` method.
    fn timeout_at(self, at: Instant) -> Timeout<Self>
        where Self::Error: From<io::Error>,
    {
        Timeout {
            timeout: Delay::new_at(at),
            future: self,
        }
    }
}

impl<F: Future> FutureExt for F {}

/// Future returned by the `FutureExt::timeout` method.
pub struct Timeout<F> {
    timeout: Delay,
    future: F,
}

impl<F> Future for Timeout<F>
    where F: Future,
          F::Error: From<io::Error>,
{
    type Item = F::Item;
    type Error = F::Error;

    fn poll(&mut self, cx: &mut task::Context) -> Poll<F::Item, F::Error> {
        match self.future.poll(cx)? {
            Async::Pending => {}
            other => return Ok(other)
        }

        if self.timeout.poll(cx)?.is_ready() {
            Err(io::Error::new(io::ErrorKind::TimedOut, "future timed out").into())
        } else {
            Ok(Async::Pending)
        }
    }
}

/// An extension trait for streams which provides convenient accessors for
/// timing out execution and such.
pub trait StreamExt: Stream + Sized {

    /// Creates a new stream which will take at most `dur` time to yield each
    /// item of the stream.
    ///
    /// This combinator creates a new stream which wraps the receiving stream
    /// in a timeout-per-item. The stream returned will resolve in at most
    /// `dur` time for each item yielded from the stream. The first item's timer
    /// starts when this method is called.
    ///
    /// If a stream's item completes before `dur` elapses then the timer will be
    /// reset for the next item. If the timeout elapses, however, then an error
    /// will be yielded on the stream and the timer will be reset.
    fn timeout(self, dur: Duration) -> TimeoutStream<Self>
        where Self::Error: From<io::Error>,
    {
        TimeoutStream {
            timeout: Delay::new(dur),
            dur,
            stream: self,
        }
    }
}

impl<S: Stream> StreamExt for S {}

/// Stream returned by the `StreamExt::timeout` method.
pub struct TimeoutStream<S> {
    timeout: Delay,
    dur: Duration,
    stream: S,
}

impl<S> Stream for TimeoutStream<S>
    where S: Stream,
          S::Error: From<io::Error>,
{
    type Item = S::Item;
    type Error = S::Error;

    fn poll_next(&mut self, cx: &mut task::Context) -> Poll<Option<S::Item>, S::Error> {
        match self.stream.poll_next(cx) {
            Ok(Async::Pending) => {}
            other => {
                self.timeout.reset(self.dur);
                return other
            }
        }

        if self.timeout.poll(cx)?.is_ready() {
            self.timeout.reset(self.dur);
            Err(io::Error::new(io::ErrorKind::TimedOut, "stream item timed out").into())
        } else {
            Ok(Async::Pending)
        }
    }
}
