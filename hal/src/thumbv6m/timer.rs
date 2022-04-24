//! Working with timer counter hardware
use crate::ehal::timer::{CountDown, Periodic};
#[cfg(feature = "samd11")]
use crate::pac::tc1::COUNT16;
#[cfg(feature = "samd21")]
use crate::pac::tc3::COUNT16;
#[allow(unused)]
#[cfg(feature = "samd11")]
use crate::pac::{PM, TC1};
#[allow(unused)]
#[cfg(feature = "samd21")]
use crate::pac::{PM, TC3, TC4, TC5};
use crate::timer_params::TimerParams;

use crate::clock;
use crate::time::{Hertz, Nanoseconds};
use crate::timer_traits::InterruptDrivenTimer;
use void::Void;

// Note:
// TC3 + TC4 can be paired to make a 32-bit counter
// TC5 + TC6 can be paired to make a 32-bit counter

/// A generic hardware timer counter.
/// The counters are exposed in 16-bit mode only.
/// The hardware allows configuring the 8-bit mode
/// and pairing up some instances to run in 32-bit
/// mode, but that functionality is not currently
/// exposed by this hal implementation.
/// TimerCounter implements both the `Periodic` and
/// the `CountDown` embedded_hal timer traits.
/// Before a hardware timer can be used, it must first
/// have a clock configured.
pub struct TimerCounter<TC> {
    freq: Hertz,
    tc: TC,
}

/// This is a helper trait to make it easier to make most of the
/// TimerCounter impl generic.  It doesn't make too much sense to
/// to try to implement this trait outside of this module.
pub trait Count16 {
    fn count_16(&self) -> &COUNT16;
}

impl<TC> Periodic for TimerCounter<TC> {}
impl<TC> CountDown for TimerCounter<TC>
where
    TC: Count16,
{
    type Time = Nanoseconds;

    fn start<T>(&mut self, timeout: T)
    where
        T: Into<Self::Time>,
    {
        let params = TimerParams::new_us(timeout, self.freq.0);
        let divider = params.divider;
        let cycles = params.cycles;

        let count = self.tc.count_16();

        // Disable the timer while we reconfigure it
        count.ctrla.modify(|_, w| w.enable().clear_bit());
        while count.status.read().syncbusy().bit_is_set() {}

        // Now that we have a clock routed to the peripheral, we
        // can ask it to perform a reset.
        count.ctrla.write(|w| w.swrst().set_bit());
        while count.status.read().syncbusy().bit_is_set() {}
        // the SVD erroneously marks swrst as write-only, so we
        // need to manually read the bit here
        while count.ctrla.read().bits() & 1 != 0 {}

        count.ctrlbset.write(|w| {
            // Count up when the direction bit is zero
            w.dir().clear_bit();
            // Periodic
            w.oneshot().clear_bit()
        });

        // Set TOP value for mfrq mode
        count.cc[0].write(|w| unsafe { w.cc().bits(cycles as u16) });

        count.ctrla.modify(|_, w| {
            match divider {
                1 => w.prescaler().div1(),
                2 => w.prescaler().div2(),
                4 => w.prescaler().div4(),
                8 => w.prescaler().div8(),
                16 => w.prescaler().div16(),
                64 => w.prescaler().div64(),
                256 => w.prescaler().div256(),
                1024 => w.prescaler().div1024(),
                _ => unreachable!(),
            };
            // Enable Match Frequency Waveform generation
            w.wavegen().mfrq();
            w.enable().set_bit();
            w.runstdby().set_bit()
        });
    }

    fn wait(&mut self) -> nb::Result<(), Void> {
        let count = self.tc.count_16();
        if count.intflag.read().ovf().bit_is_set() {
            // Writing a 1 clears the flag
            count.intflag.modify(|_, w| w.ovf().set_bit());
            Ok(())
        } else {
            Err(nb::Error::WouldBlock)
        }
    }
}

impl<TC> InterruptDrivenTimer for TimerCounter<TC>
where
    TC: Count16,
{
    /// Enable the interrupt generation for this hardware timer.
    /// This method only sets the clock configuration to trigger
    /// the interrupt; it does not configure the interrupt controller
    /// or define an interrupt handler.
    fn enable_interrupt(&mut self) {
        self.tc.count_16().intenset.write(|w| w.ovf().set_bit());
    }

    /// Disables interrupt generation for this hardware timer.
    /// This method only sets the clock configuration to prevent
    /// triggering the interrupt; it does not configure the interrupt
    /// controller.
    fn disable_interrupt(&mut self) {
        self.tc.count_16().intenclr.write(|w| w.ovf().set_bit());
    }
}

macro_rules! tc {
    ($($TYPE:ident: ($TC:ident, $pm:ident, $clock:ident),)+) => {
        $(
pub type $TYPE = TimerCounter<$TC>;

impl Count16 for $TC {
    fn count_16(&self) -> &COUNT16 {
        self.count16()
    }
}

impl TimerCounter<$TC>
{
    /// Configure this timer counter instance.
    /// The clock is obtained from the `GenericClockController` instance
    /// and its frequency impacts the resolution and maximum range of
    /// the timeout values that can be passed to the `start` method.
    /// Note that some hardware timer instances share the same clock
    /// generator instance and thus will be clocked at the same rate.
    pub fn $pm(clock: &clock::$clock, tc: $TC, pm: &mut PM) -> Self {
        // this is safe because we're constrained to just the tc3 bit
        pm.apbcmask.modify(|_, w| w.$pm().set_bit());
        {
            let count = tc.count_16();

            // Disable the timer while we reconfigure it
            count.ctrla.modify(|_, w| w.enable().clear_bit());
            while count.status.read().syncbusy().bit_is_set() {}
        }
        Self {
            freq: clock.freq(),
            tc,
        }
    }
}
        )+
    }
}

// samd11
#[cfg(feature = "samd11")]
tc! {
    TimerCounter1: (TC1, tc1_, Tc1Tc2Clock),
}
// samd21
#[cfg(feature = "samd21")]
tc! {
    TimerCounter3: (TC3, tc3_, Tcc2Tc3Clock),
    TimerCounter4: (TC4, tc4_, Tc4Tc5Clock),
    TimerCounter5: (TC5, tc5_, Tc4Tc5Clock),
}

#[cfg(feature = "async")]
pub mod async_timer {

    use super::*;
    use atomic_polyfill::AtomicBool;
    use core::{
        sync::atomic::Ordering,
        task::{Poll, Waker},
    };
    use embassy::{interrupt::InterruptExt, waitqueue::AtomicWaker};
    use futures::future::poll_fn;

    pub trait AsyncCount16: Count16 {
        type Interrupt: ::embassy::interrupt::InterruptExt;
        const STATE_ID: usize;
        unsafe fn on_interrupt(_: *mut ());
    }

    macro_rules! impl_async_count16 {
        ($(($TC: ident, $id: expr)),+) => {
            $(
                impl AsyncCount16 for $TC {
                    type Interrupt = crate::interrupt::$TC;
                    const STATE_ID: usize = $id;

                    unsafe fn on_interrupt(_: *mut ()) {
                        let tc = crate::pac::Peripherals::steal().$TC;
                        let intflag = &tc.count_16().intflag;

                        if intflag.read().ovf().bit_is_set() {
                            // Clear the flag
                            intflag.modify(|_, w| w.ovf().set_bit());
                            STATE[Self::STATE_ID].wake();
                        }
                    }
                }
            )+
        };
    }

    #[cfg(feature = "samd11")]
    impl_async_count16!((TC1, 0));

    #[cfg(feature = "samd21")]
    impl_async_count16!((TC3, 0), (TC4, 1), (TC5, 2));

    impl<TC: AsyncCount16> TimerCounter<TC> {
        pub fn into_future<'a, 'self_mut: 'a>(
            &'self_mut mut self,
            irq: &'a mut TC::Interrupt,
        ) -> AsyncTimer<'a, TC> {
            AsyncTimer::new(self, irq)
        }
    }

    // TODO instead of tracking the state manually, we could use ONESHOT
    // mode and check the STATUS.STOP bit
    struct State {
        waker: AtomicWaker,
        ready: AtomicBool,
    }

    impl State {
        const fn new() -> Self {
            Self {
                waker: AtomicWaker::new(),
                ready: AtomicBool::new(false),
            }
        }

        fn register(&self, waker: &Waker) {
            self.waker.register(waker)
        }

        fn wake(&self) {
            self.ready.store(true, Ordering::SeqCst);
            self.waker.wake()
        }

        fn ready(&self) -> bool {
            self.ready.swap(false, Ordering::SeqCst)
        }
    }

    const STATE_NEW: State = State::new();
    static STATE: [State; 3] = [STATE_NEW; 3];

    pub struct AsyncTimer<'a, TC>
    where
        TC: AsyncCount16,
    {
        timer: &'a mut TimerCounter<TC>,
        irq: &'a mut TC::Interrupt,
    }

    impl<'a, TC> AsyncTimer<'a, TC>
    where
        TC: AsyncCount16,
    {
        pub fn new(timer: &'a mut TimerCounter<TC>, irq: &'a mut TC::Interrupt) -> Self {
            irq.set_handler(TC::on_interrupt);
            irq.enable();
            timer.enable_interrupt();

            Self { timer, irq }
        }

        pub async fn delay_ms(&mut self, count: impl Into<Nanoseconds>) {
            self.timer.start(count);
            self.timer.enable_interrupt();

            poll_fn(|cx| {
                STATE[TC::STATE_ID].register(cx.waker());
                if STATE[TC::STATE_ID].ready() {
                    return Poll::Ready(());
                }

                Poll::Pending
            })
            .await;
        }
    }

    impl<'a, TC: AsyncCount16> Drop for AsyncTimer<'a, TC> {
        fn drop(&mut self) {
            self.irq.remove_handler();
            self.irq.disable();
        }
    }
}
