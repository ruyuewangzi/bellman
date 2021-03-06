//! An interface for dealing with the kinds of parallel computations involved in
//! `bellperson`. It's currently just a thin wrapper around [`CpuPool`] and
//! [`rayon`] but may be extended in the future to allow for various
//! parallelism strategies.
//!
//! [`CpuPool`]: futures_cpupool::CpuPool

#[cfg(feature = "multicore")]
mod implementation {
    use futures::{Future, IntoFuture, Poll};
    use futures_cpupool::{CpuFuture, CpuPool};
    use lazy_static::lazy_static;
    use num_cpus;
    use std::env;

    lazy_static! {
        static ref NUM_CPUS: usize = if let Ok(num) = env::var("BELLMAN_NUM_CPUS") {
            if let Ok(num) = num.parse() {
                num
            } else {
                num_cpus::get()
            }
        } else {
            num_cpus::get()
        };
        pub static ref THREAD_POOL: rayon::ThreadPool = rayon::ThreadPoolBuilder::new()
            .num_threads(*NUM_CPUS)
            .build()
            .unwrap();
        static ref CPU_POOL: CpuPool = CpuPool::new(*NUM_CPUS);
    }

    #[derive(Clone)]
    pub struct Worker {}

    impl Worker {
        pub fn new() -> Worker {
            Worker {}
        }

        pub fn log_num_cpus(&self) -> u32 {
            log2_floor(*NUM_CPUS)
        }

        pub fn compute<F, R>(&self, f: F) -> WorkerFuture<R::Item, R::Error>
        where
            F: FnOnce() -> R + Send + 'static,
            R: IntoFuture + 'static,
            R::Future: Send + 'static,
            R::Item: Send + 'static,
            R::Error: Send + 'static,
        {
            WorkerFuture {
                future: CPU_POOL.spawn_fn(f),
            }
        }

        pub fn scope<'a, F, R>(&self, elements: usize, f: F) -> R
        where
            F: FnOnce(&rayon::Scope<'a>, usize) -> R + Send,
            R: Send,
        {
            let chunk_size = if elements < *NUM_CPUS {
                1
            } else {
                elements / *NUM_CPUS
            };

            THREAD_POOL.scope(|scope| f(scope, chunk_size))
        }
    }

    pub struct WorkerFuture<T, E> {
        future: CpuFuture<T, E>,
    }

    impl<T: Send + 'static, E: Send + 'static> Future for WorkerFuture<T, E> {
        type Item = T;
        type Error = E;

        fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
            self.future.poll()
        }
    }

    fn log2_floor(num: usize) -> u32 {
        assert!(num > 0);

        let mut pow = 0;

        while (1 << (pow + 1)) <= num {
            pow += 1;
        }

        pow
    }

    #[test]
    fn test_log2_floor() {
        assert_eq!(log2_floor(1), 0);
        assert_eq!(log2_floor(2), 1);
        assert_eq!(log2_floor(3), 1);
        assert_eq!(log2_floor(4), 2);
        assert_eq!(log2_floor(5), 2);
        assert_eq!(log2_floor(6), 2);
        assert_eq!(log2_floor(7), 2);
        assert_eq!(log2_floor(8), 3);
    }
}

#[cfg(not(feature = "multicore"))]
mod implementation {
    use futures::{future, Future, IntoFuture, Poll};

    #[derive(Clone)]
    pub struct Worker;

    impl Worker {
        pub fn new() -> Worker {
            Worker
        }

        pub fn log_num_cpus(&self) -> u32 {
            0
        }

        pub fn compute<F, R>(&self, f: F) -> R::Future
        where
            F: FnOnce() -> R + Send + 'static,
            R: IntoFuture + 'static,
            R::Future: Send + 'static,
            R::Item: Send + 'static,
            R::Error: Send + 'static,
        {
            f().into_future()
        }

        pub fn scope<F, R>(&self, elements: usize, f: F) -> R
        where
            F: FnOnce(&DummyScope, usize) -> R,
        {
            f(&DummyScope, elements)
        }
    }

    pub struct WorkerFuture<T, E> {
        future: future::FutureResult<T, E>,
    }

    impl<T: Send + 'static, E: Send + 'static> Future for WorkerFuture<T, E> {
        type Item = T;
        type Error = E;

        fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
            self.future.poll()
        }
    }

    pub struct DummyScope;

    impl DummyScope {
        pub fn spawn<F: FnOnce(&DummyScope)>(&self, f: F) {
            f(self);
        }
    }
}

pub use self::implementation::*;
