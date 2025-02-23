use crate::lib::*;
use crate::{algorithms::Algorithm, instant, NegativeMultiDecision};

/// A representation of a bare in-memory algorithm, without any bucket
/// attached.
#[derive(Debug)]
pub struct AlgorithmForTest<A: Algorithm<P>, P: instant::Relative>(A, PhantomData<P>);

impl<'a, A, P> AlgorithmForTest<A, P>
where
    A: Algorithm<P>,
    P: instant::Relative,
{
    pub fn new<U: Into<Option<NonZeroU32>>, D: Into<Option<Duration>>>(
        cap: NonZeroU32,
        weight: U,
        duration: D,
    ) -> Self {
        AlgorithmForTest(
            A::construct(
                cap,
                weight.into().unwrap_or(nonzero!(1u32)),
                duration
                    .into()
                    .unwrap_or(crate::lib::Duration::from_secs(1)),
            )
            .unwrap(),
            PhantomData,
        )
    }

    pub fn algorithm(&'a self) -> &'a A {
        &self.0
    }

    pub fn state(&self) -> A::BucketState {
        A::BucketState::default()
    }

    pub fn check(&self, state: &A::BucketState, t0: P) -> Result<(), A::NegativeDecision> {
        self.0.test_and_update(state, t0)
    }

    pub fn check_n(
        &self,
        state: &A::BucketState,
        n: u32,
        t0: P,
    ) -> Result<(), NegativeMultiDecision<A::NegativeDecision>> {
        self.0.test_n_and_update(state, n, t0)
    }
}

impl<A, P> Default for AlgorithmForTest<A, P>
where
    A: Algorithm<P>,
    P: instant::Relative,
{
    fn default() -> Self {
        Self::new(nonzero!(1u32), None, None)
    }
}

#[macro_export]
#[doc(hidden)]
macro_rules! bench_with_algorithm_variants {
    ($variant:expr, $var:ident, $code:block) => {
        match $variant {
            $crate::test_utilities::variants::Variant::GCRA => {
                let mut $var = $crate::test_utilities::algorithms::AlgorithmForTest::<
                    $crate::GCRA<Instant>,
                    Instant,
                >::default();
                $code
            }
            $crate::test_utilities::variants::Variant::LeakyBucket => {
                let mut $var = $crate::test_utilities::algorithms::AlgorithmForTest::<
                    $crate::LeakyBucket<Instant>,
                    Instant,
                >::default();
                $code
            }
        }
    };
}
