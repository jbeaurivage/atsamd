use crate::{
    ehal::timer::CountDown, time::Nanoseconds, timer_traits::InterruptDrivenTimer,
    typelevel::Sealed,
};
use atomic_polyfill::AtomicBool;
use core::{
    future::poll_fn,
    sync::atomic::Ordering,
    task::{Poll, Waker},
};
use cortex_m::interrupt::InterruptNumber;
use cortex_m_interrupt::NvicInterruptHandle;
use embassy_sync::waitqueue::AtomicWaker;

#[cfg(any(feature = "samd11", feature = "samd21"))]
use crate::thumbv6m::timer;

#[cfg(feature = "min-samd51g")]
use crate::thumbv7em::timer;

#[cfg(feature = "samd11")]
use crate::pac::TC1;
#[cfg(feature = "samd21")]
use crate::pac::{TC3, TC4, TC5};

#[cfg(feature = "min-samd51g")]
use crate::pac::{TC2, TC3};
// Only the G variants are missing these timers
#[cfg(feature = "min-samd51j")]
use crate::pac::{TC4, TC5};

use timer::{Count16, TimerCounter};

#[doc(hidden)]
pub trait AsyncCount16: Count16 + Sealed {
    /// Index of this TC in the `STATE` tracker
    const STATE_ID: usize;

    /// Callback function when the corresponding TC interrupt is fired
    ///
    /// # Safety
    ///
    /// This method may [`steal`](crate::pac::Peripherals::steal) the `TC`
    /// peripheral instance to check the interrupt flags. The only
    /// modifications it is allowed to apply to the peripheral is to clear
    /// the interrupt flag (to prevent re-firing). This method should ONLY be
    /// able to be called while an [`AsyncTimer`] holds an unique reference
    /// to the underlying `TC` peripheral.
    fn on_interrupt();
}

macro_rules! impl_async_count16 {
        ($(($TC: ident, $id: expr)),+) => {
            $(
                impl AsyncCount16 for $TC {
                    const STATE_ID: usize = $id;

                    fn on_interrupt() {
                        let tc = unsafe{ crate::pac::Peripherals::steal().$TC};
                        let intflag = &tc.count_16().intflag;

                        if intflag.read().ovf().bit_is_set() {
                            // Clear the flag
                            intflag.modify(|_, w| w.ovf().set_bit());
                            STATE[Self::STATE_ID].wake();
                        }
                    }
                }

                impl Sealed for $TC {}
            )+
        };
    }

#[cfg(feature = "samd11")]
impl_async_count16!((TC1, 0));
#[cfg(feature = "samd11")]
const NUM_TIMERS: usize = 1;

#[cfg(feature = "samd21")]
impl_async_count16!((TC3, 0), (TC4, 1), (TC5, 2));
#[cfg(feature = "samd21")]
const NUM_TIMERS: usize = 3;

#[cfg(feature = "samd51g")]
impl_async_count16!((TC2, 0), (TC3, 1));
#[cfg(feature = "samd51g")]
const NUM_TIMERS: usize = 2;

#[cfg(feature = "min-samd51j")]
impl_async_count16!((TC2, 0), (TC3, 1), (TC4, 2), (TC5, 3));
#[cfg(feature = "min-samd51j")]
const NUM_TIMERS: usize = 4;

impl<T> TimerCounter<T>
where
    T: AsyncCount16,
{
    /// Transform a [`TimerCounter`] into an [`AsyncTimer`]
    #[inline]
    pub fn into_async<I, N>(mut self, irq: I) -> AsyncTimer<T, N>
    where
        I: NvicInterruptHandle<N>,
        N: InterruptNumber,
    {
        let irq_number = irq.number();
        irq.register(T::on_interrupt);
        unsafe { cortex_m::peripheral::NVIC::unmask(irq_number) };
        self.enable_interrupt();

        AsyncTimer {
            timer: self,
            irq_number,
        }
    }
}

/// Wrapper around a [`TimerCounter`] with an `async` interface
pub struct AsyncTimer<T, I>
where
    T: AsyncCount16,
    I: InterruptNumber,
{
    timer: TimerCounter<T>,
    irq_number: I,
}

impl<T, I> AsyncTimer<T, I>
where
    T: AsyncCount16,
    I: InterruptNumber,
{
    /// Delay asynchronously
    #[inline]
    pub async fn delay(&mut self, count: impl Into<Nanoseconds>) {
        self.timer.start(count);
        self.timer.enable_interrupt();

        poll_fn(|cx| {
            STATE[T::STATE_ID].register(cx.waker());
            if STATE[T::STATE_ID].ready() {
                return Poll::Ready(());
            }

            Poll::Pending
        })
        .await;
    }
}

impl<T, I> Drop for AsyncTimer<T, I>
where
    T: AsyncCount16,
    I: InterruptNumber,
{
    #[inline]
    fn drop(&mut self) {
        cortex_m::peripheral::NVIC::mask(self.irq_number);
    }
}

// TODO instead of tracking the state manually, we could use ONESHOT
// mode and check the STATUS.STOP bit
struct State {
    waker: AtomicWaker,
    ready: AtomicBool,
}

impl State {
    #[inline]
    const fn new() -> Self {
        Self {
            waker: AtomicWaker::new(),
            ready: AtomicBool::new(false),
        }
    }

    #[inline]
    fn register(&self, waker: &Waker) {
        self.waker.register(waker)
    }

    #[inline]
    fn wake(&self) {
        self.ready.store(true, Ordering::SeqCst);
        self.waker.wake()
    }

    #[inline]
    fn ready(&self) -> bool {
        self.ready.swap(false, Ordering::SeqCst)
    }
}

#[allow(clippy::declare_interior_mutable_const)]
const STATE_NEW: State = State::new();
static STATE: [State; NUM_TIMERS] = [STATE_NEW; NUM_TIMERS];