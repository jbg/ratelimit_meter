pub mod gcra;
pub mod leaky_bucket;

pub use self::gcra::*;
pub use self::leaky_bucket::*;

use crate::{instant, InconsistentCapacity, NegativeMultiDecision};

use crate::lib::*;

/// The default rate limiting algorithm in this crate: The ["leaky
/// bucket"](leaky_bucket/struct.LeakyBucket.html).
///
/// The leaky bucket algorithm is fairly easy to understand and has
/// decent performance in most cases. If better threaded performance
/// is needed, this crate also offers the
/// [`GCRA`](gcra/struct.GCRA.html) algorithm.
pub type DefaultAlgorithm = LeakyBucket;

/// Provides additional information about non-conforming cells, most
/// importantly the earliest time until the next cell could be
/// considered conforming.
///
/// Since this does not account for effects like thundering herds,
/// users should always add random jitter to the times given.
pub trait NonConformance<P: instant::Relative = instant::TimeSource> {
    /// Returns the earliest time at which a decision could be
    /// conforming (excluding conforming decisions made by the Decider
    /// that are made in the meantime).
    fn earliest_possible(&self) -> P;

    /// Returns the minimum amount of time from the time that the
    /// decision was made (relative to the `at` argument in a
    /// `Decider`'s `check_at` method) that must pass before a
    /// decision can be conforming. Since Durations can not be
    /// negative, a zero duration is returned if `from` is already
    /// after that duration.
    fn wait_time_from(&self, from: P) -> Duration {
        let earliest = self.earliest_possible();
        earliest.duration_since(earliest.min(from))
    }
}

pub trait NonConformanceExt<P: instant::Absolute>: NonConformance<P> {
    /// Returns the minimum amount of time (down to 0) that needs to
    /// pass from the current instant for the Decider to consider a
    /// cell conforming again.
    fn wait_time(&self) -> Duration {
        self.wait_time_from(P::now())
    }
}

impl<P: instant::Absolute, T> NonConformanceExt<P> for T where T: NonConformance<P> {}

/// The trait that implementations of metered rate-limiter algorithms
/// have to implement.
///
/// Implementing structures are expected to represent the "parameters"
/// (e.g., the allowed requests/s), and keep the information necessary
/// to make a decision, e.g. concrete usage statistics for an
/// in-memory rate limiter, in the associated structure
/// [`BucketState`](#associatedtype.BucketState).
pub trait Algorithm<P: instant::Relative = instant::TimeSource>:
    Send + Sync + Sized + fmt::Debug
{
    /// The state of a single rate limiting bucket.
    ///
    /// Every new rate limiting state is initialized as `Default`. The
    /// states must be safe to share across threads (this crate uses a
    /// `parking_lot` Mutex to allow that).
    type BucketState: RateLimitState<Self, P>;

    /// The type returned when a rate limiting decision for a single
    /// cell is negative. Each rate limiting algorithm can decide to
    /// return the type that suits it best, but most algorithms'
    /// decisions also implement
    /// [`NonConformance`](trait.NonConformance.html), to ease
    /// handling of how long to wait.
    type NegativeDecision: PartialEq + fmt::Display + fmt::Debug + Send + Sync;

    /// Constructs a rate limiter with the given parameters:
    /// `capacity` is the number of cells to allow, weighing
    /// `cell_weight`, every `per_time_unit`.
    fn construct(
        capacity: NonZeroU32,
        cell_weight: NonZeroU32,
        per_time_unit: Duration,
    ) -> Result<Self, InconsistentCapacity>;

    /// Tests if `n` cells can be accommodated in the rate limiter at
    /// the instant `at` and updates the rate-limiter state to account
    /// for the weight of the cells and updates the ratelimiter state.
    ///
    /// The update is all or nothing: Unless all n cells can be
    /// accommodated, the state of the rate limiter will not be
    /// updated.
    fn test_n_and_update(
        &self,
        state: &Self::BucketState,
        n: u32,
        at: P,
    ) -> Result<(), NegativeMultiDecision<Self::NegativeDecision>>;

    /// Tests if a single cell can be accommodated in the rate limiter
    /// at the instant `at` and updates the rate-limiter state to
    /// account for the weight of the cell.
    ///
    /// This method is provided by default, using the `n` test&update
    /// method.
    fn test_and_update(
        &self,
        state: &Self::BucketState,
        at: P,
    ) -> Result<(), Self::NegativeDecision> {
        match self.test_n_and_update(state, 1, at) {
            Ok(()) => Ok(()),
            Err(NegativeMultiDecision::BatchNonConforming(1, nc)) => Err(nc),
            Err(other) => unreachable!(
                "BUG: measuring a batch of size 1 reported insufficient capacity: {:?}",
                other
            ),
        }
    }
}

/// Trait that all rate limit states have to implement around
/// housekeeping in keyed rate limiters.
pub trait RateLimitState<P, I: instant::Relative>: Default + Send + Sync + Eq + fmt::Debug {}

/// Trait that all rate limit states implement if there is a real-time
/// clock available.
pub trait RateLimitStateWithClock<P, I: instant::Absolute>: RateLimitState<P, I> {
    /// Returns the last time instant that the state had any relevance
    /// (i.e. the rate limiter would behave exactly as if it was a new
    /// rate limiter after this time).
    ///
    /// If the state has not been touched for a given amount of time,
    /// the keyed rate limiter will expire it.
    ///
    /// # Thread safety
    /// This uses a bucket state snapshot to determine eligibility;
    /// race conditions can occur.
    fn last_touched(&self, params: &P) -> I;
}

#[cfg(feature = "std")]
mod std {
    use crate::instant;
    use evmap::ShallowCopy;

    /// Trait implemented by all rate limit states that are compatible
    /// with the KeyedRateLimiters.
    pub trait KeyableRateLimitState<P, I: instant::Absolute>:
        super::RateLimitStateWithClock<P, I> + ShallowCopy
    {
    }

    #[cfg(feature = "std")]
    impl<T, P, I> KeyableRateLimitState<P, I> for T
    where
        T: super::RateLimitStateWithClock<P, I> + ShallowCopy,
        I: instant::Absolute,
    {
    }
}

#[cfg(feature = "std")]
pub use self::std::*;
