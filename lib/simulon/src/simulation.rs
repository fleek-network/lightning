use std::any::Any;
use std::cell::UnsafeCell;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread::JoinHandle;
use std::time::Duration;

use indicatif::ProgressBar;
use triomphe::Arc;

use crate::latency::{DefaultLatencyProvider, LatencyProvider};
use crate::message::Message;
use crate::report::{Metrics, Report};
use crate::state::{NodeState, hook_node, with_node};
use crate::storage::TypedStorage;
use crate::{FRAME_DURATION, FRAME_TO_MS};

/// Constructor for a simulation which allows you to set the parameters of a simulation.
pub struct SimulationBuilder<L = DefaultLatencyProvider> {
    executor: Box<dyn Fn() + Send + Sync>,
    num_workers: Option<usize>,
    num_nodes: Option<usize>,
    frame_per_node_report: usize,
    frame_per_global_report: usize,
    storage: TypedStorage,
    latency_provider: Option<L>,
    show_progress: bool,
}

pub struct Simulation<L: LatencyProvider = DefaultLatencyProvider> {
    /// The current time in nanoseconds.
    now: u128,
    /// The shared state between us and the workers.
    state: Arc<SharedState>,
    /// The owned array of nodes in their actual order.
    nodes: Box<[NodeState]>,
    /// A handle to each worker thread.
    workers: Vec<JoinHandle<()>>,
    /// The latency provider.
    latency_provider: L,
    /// Show progress bar or not.
    show_progress: bool,
}

#[derive(Default)]
struct WorkerState {
    /// For each worker we store the list of messages their nodes wants to send out.
    outgoing: Vec<Message>,
    /// The collected metrics on this worker.
    metrics: Report,
}

struct SharedState {
    /// The executor function for each task.
    executor: Box<dyn Fn() + Send + Sync>,
    /// Number of frames for each report on each node.
    frame_per_node_report: usize,
    /// Number of frames for each global report.
    frame_per_global_report: usize,
    /// The state for each worker. We use an `UnsafeCell` instead of a Mutex since we know
    /// that our synchronization strategy already guarantees that the worker state is either:
    ///
    /// 1. Accessed by main thread after a frame is executed by every worker.
    /// 2. Accessed by a worker thread during the execution of a frame.
    ///
    /// So only one thread (`main/worker`) is interested in this data at a time.
    workers: Box<[UnsafeCell<WorkerState>]>,
    /// Nodes sorted by their event time.
    nodes: Box<[*mut NodeState]>,
    /// The current frame.
    frame: AtomicUsize,
    /// The current node that is being processed.
    cursor: AtomicUsize,
    /// Number of threads that have done their execution and are ready to start the next frame.
    ready_workers: AtomicUsize,
}

// Because `SyncUnsafeCell` is unstable and nightly.
unsafe impl Sync for SharedState {}
unsafe impl Send for SharedState {}

impl SimulationBuilder {
    /// Creates a new simulation builder with the provided executor function. The executor function
    /// is used to drive the state of the simulated node.
    ///
    /// It should be a pure function that only uses the [`simulon::api`] functions to perform I/O
    /// with other simulated nodes.
    pub fn new<E>(executor: E) -> Self
    where
        E: Fn() + Send + Sync + 'static,
    {
        Self {
            executor: Box::new(executor),
            num_workers: None,
            num_nodes: None,
            frame_per_node_report: (FRAME_TO_MS * 10) as usize,
            frame_per_global_report: FRAME_TO_MS as usize,
            storage: TypedStorage::default(),
            latency_provider: None,
            show_progress: false,
        }
    }
}

impl<L> SimulationBuilder<L> {
    /// Inject the given value as shared state value for the executor to access.
    pub fn with_state<T: Any + Send + Sync>(mut self, data: T) -> Self {
        self.storage.insert(data);
        self
    }

    /// Show a progress bar when running the simulation.
    pub fn enable_progress_bar(mut self) -> Self {
        self.show_progress = true;
        self
    }

    /// Determines the number of workers that we should use to run this simulation.
    ///
    /// # Panics
    ///
    /// If the value zero is passed.
    ///
    /// # Default
    ///
    /// By default it equals to `num_cpus::get_physical() - 1`.
    pub fn with_workers(mut self, n: usize) -> Self {
        assert!(n > 0, "Number of workers must be greater than 0");
        self.num_workers = Some(n);
        self
    }

    /// Determines the number of instances that we should simulate.
    ///
    /// # Panics
    ///
    /// If the value zero is passed.
    ///
    /// # Default
    ///
    /// By default the number of nodes is 4 times the number of workers.
    pub fn with_nodes(mut self, n: usize) -> Self {
        assert!(n > 0, "Number of nodes must be greater than 0");
        self.num_nodes = Some(n);
        self
    }

    /// Sets the compaction rate of the collected metrics per each individual node. Use `0` to not
    /// collect per-frame metric data on each node.
    ///
    /// # Default
    ///
    /// Default value is `10ms`.
    pub fn set_node_metrics_rate(mut self, duration: Duration) -> Self {
        let rate = duration.as_nanos() / FRAME_DURATION.as_nanos();
        assert!(rate < (usize::MAX as u128));
        self.frame_per_node_report = rate as usize;
        self
    }

    /// Sets the compaction rate of the globally aggregated collected metrics collect per-frame
    /// metric data on each node.
    ///
    /// # Default
    ///
    /// Default value is `1ms`.
    pub fn set_global_metrics_rate(mut self, duration: Duration) -> Self {
        let rate = duration.as_nanos() / FRAME_DURATION.as_nanos();
        assert!(rate < (usize::MAX as u128));
        self.frame_per_global_report = rate as usize;
        self
    }

    /// Set a custom instance of a latency provider.
    pub fn set_latency_provider<T: LatencyProvider>(self, provider: T) -> SimulationBuilder<T> {
        SimulationBuilder {
            executor: self.executor,
            num_workers: self.num_workers,
            num_nodes: self.num_nodes,
            frame_per_node_report: self.frame_per_node_report,
            frame_per_global_report: self.frame_per_global_report,
            storage: self.storage,
            latency_provider: Some(provider),
            show_progress: self.show_progress,
        }
    }

    pub fn build(self) -> Simulation<L>
    where
        L: LatencyProvider,
    {
        let num_workers = self
            .num_workers
            .unwrap_or_else(|| num_cpus::get_physical() - 1)
            .max(1);
        let num_nodes = self.num_nodes.unwrap_or(num_workers * 4);

        // Cap the number of workers to the number of nodes.
        let num_workers = num_workers.min(num_nodes);
        let storage = Arc::new(self.storage);
        let nodes = (0..num_nodes)
            .map(|i| NodeState::new(storage.clone(), num_nodes, i))
            .collect::<Vec<_>>()
            .into_boxed_slice();

        let ptr = nodes.as_ptr();

        let state = SharedState {
            executor: self.executor,
            frame_per_node_report: self.frame_per_node_report,
            frame_per_global_report: self.frame_per_global_report,
            workers: (0..num_workers)
                .map(|_| {
                    let mut worker = WorkerState::default();
                    worker.outgoing.reserve(num_nodes / num_workers * 16);
                    UnsafeCell::new(worker)
                })
                .collect::<Vec<_>>()
                .into_boxed_slice(),
            nodes: nodes
                .iter()
                .enumerate()
                .map(|(i, _)| unsafe { ptr.add(i) as *mut NodeState })
                .collect::<Vec<_>>()
                .into_boxed_slice(),
            frame: AtomicUsize::new(0),
            cursor: AtomicUsize::new(0),
            ready_workers: AtomicUsize::new(0),
        };

        Simulation {
            now: 0,
            state: Arc::new(state),
            nodes,
            workers: Vec::with_capacity(num_workers),
            latency_provider: self.latency_provider.unwrap_or_default(),
            show_progress: self.show_progress,
        }
    }

    /// Build and run the simulation.
    pub fn run(self, duration: Duration) -> Report
    where
        L: LatencyProvider,
    {
        self.build().run(duration)
    }
}

impl<L: LatencyProvider> Simulation<L> {
    pub fn run(mut self, duration: Duration) -> Report {
        // Initialize the latency provider.
        self.latency_provider.init(self.nodes.len());

        self.start_threads();

        let mut n = duration.as_nanos() / FRAME_DURATION.as_nanos();
        let pb = self.show_progress.then(|| ProgressBar::new(n as u64));

        // Run frame zero regardless that the event queue is empty.
        wait_for_workers(&self.state);
        self.state.ready_workers.store(0, Ordering::Relaxed);
        self.state.frame.fetch_add(1, Ordering::Relaxed);
        if let Some(pb) = pb.as_ref() {
            pb.inc(1);
        }

        while n > 1 {
            // wait for the workers to become online.
            wait_for_workers(&self.state);

            // reset the work stealing state.
            self.state.ready_workers.store(0, Ordering::Relaxed);
            self.state.cursor.store(0, Ordering::Relaxed);

            // Run the post executing tasks and figure out how many frames we should move forward.
            if let Some(skip) = self.run_post_frame() {
                if n <= skip as u128 {
                    break;
                }

                debug_assert!(skip >= 1);

                // Move the clock to `skip` frames forward.
                self.now += skip as u128 * FRAME_DURATION.as_nanos();

                // Update the loop counter and move to the frame.
                n -= skip as u128;
                self.state.frame.fetch_add(skip, Ordering::Relaxed);
                if let Some(pb) = pb.as_ref() {
                    pb.inc(skip as u64);
                }
            } else {
                // End early since there is no more event to be processed.
                break;
            }
        }

        if let Some(pb) = pb.as_ref() {
            pb.inc(n as u64);
        }

        // wait for threads one last time.
        self.stop_threads();

        self.finish()
    }

    fn finish(mut self) -> Report {
        let mut report = self
            .state
            .workers
            .iter()
            .map(|v| unsafe { &mut *v.get() })
            .map(|s| std::mem::take(&mut s.metrics))
            .fold(Report::default(), |a, b| a + b);

        for node in self.nodes.iter_mut() {
            for (event, time) in node.emitted.drain() {
                *report
                    .log
                    .emitted
                    .entry(event)
                    .or_default()
                    .entry(time / 1_000_000)
                    .or_default() += 1;
            }

            report.node.push(std::mem::take(&mut node.metrics));
        }

        report
    }

    fn run_post_frame(&mut self) -> Option<usize> {
        // Move the messages generated by each worker to each of the destinations.
        for messages in self
            .state
            .workers
            .iter()
            .map(|s| &mut unsafe { &mut *s.get() }.outgoing)
        {
            for mut msg in messages.drain(..) {
                let node_id = msg.receiver.0;
                let latency = self
                    .latency_provider
                    .get(msg.sender.0, msg.receiver.0)
                    .as_nanos();

                debug_assert!(latency > 0);
                msg.time.0 += latency;

                self.nodes[node_id].received.push(msg);
            }
        }

        let ptr = self.state.nodes.as_ptr();
        let slice = unsafe {
            std::slice::from_raw_parts_mut(ptr as *mut *mut NodeState, self.state.nodes.len())
        };

        // Sort the nodes by the time of their first event.
        slice.sort_by_key(|k| std::cmp::Reverse(unsafe { &**k }.received.peek().map(|x| x.time)));

        // Figure out how many frames to move forward.
        let first = unsafe { &*self.state.nodes[0] };
        let msg = first.received.peek()?;
        let time = msg.time.0;

        debug_assert!(time > self.now);
        Some((time - self.now).div_ceil(FRAME_DURATION.as_nanos()).max(1) as usize)
    }

    fn start_threads(&mut self) {
        debug_assert_eq!(self.workers.len(), 0);

        let num_workers = self.state.workers.len();
        for i in 0..num_workers {
            let state = self.state.clone();
            std::thread::spawn(move || worker_loop(i, state));
        }
    }

    fn stop_threads(&mut self) {
        self.state.frame.store(usize::MAX, Ordering::Relaxed);
        while let Some(handle) = self.workers.pop() {
            handle.join().expect("Worker thread paniced.");
        }
    }
}

fn worker_loop(worker_index: usize, state: Arc<SharedState>) {
    let mut current_frame = state.frame.load(Ordering::Relaxed);

    // Safety: Our synchronization strategy guarantees that only one thread is accessing
    // this data
    let worker_state = unsafe { &mut *state.workers[worker_index].get() };

    loop {
        // Signal to everyone that we're ready to move to the next frame.
        state.ready_workers.fetch_add(1, Ordering::Relaxed);

        // If true is returned it means that we're done and should exit the thread.
        if let Some(frame) = wait_for_next_frame(&state, current_frame) {
            current_frame = frame;
        } else {
            break;
        }

        loop {
            let index = state.cursor.fetch_add(1, Ordering::Relaxed);

            if index >= state.nodes.len() {
                break;
            }

            if execute_node(&state, worker_state, current_frame - 1, index) {
                break;
            }

            hook_node(std::ptr::null_mut());
        }
    }
}

/// Returns true if the node did not have any task to be executed. Since the array of nodes
/// is sorted by tasks this indicates that the other nodes are not going to have a task as well
/// and that we can skip them.
fn execute_node(
    state: &Arc<SharedState>,
    worker_state: &mut WorkerState,
    frame: usize,
    index: usize,
) -> bool {
    let ptr = state.nodes[index];
    hook_node(ptr);

    // update the time on the node.
    let is_stalled = with_node(|n| {
        n.time = (frame as u128) * FRAME_DURATION.as_nanos();
        n.is_stalled()
    });

    if is_stalled && frame > 0 {
        return true;
    }

    let started = std::time::Instant::now();
    if frame == 0 {
        (state.executor)();
    }

    with_node(|n| {
        n.run_until_stalled();
        let elapsed = started.elapsed();
        n.current_metrics.cpu_time += elapsed.as_nanos();

        // Move the outgoing messages that this node generated to the worker's
        // outgoing message set.
        worker_state.outgoing.append(&mut n.outgoing);

        // Push the metrics for this frame to the reporter and clear the data.
        n.metrics.insert(
            frame.checked_div(state.frame_per_node_report),
            n.current_metrics,
        );

        worker_state.metrics.insert(
            frame.checked_div(state.frame_per_global_report),
            n.current_metrics,
        );

        n.current_metrics = Metrics::default();
    });

    false
}

fn wait_for_next_frame(state: &Arc<SharedState>, current_frame: usize) -> Option<usize> {
    loop {
        let frame = state.frame.load(Ordering::Relaxed);

        if frame == usize::MAX {
            return None;
        }

        if frame > current_frame {
            return Some(frame);
        }

        std::hint::spin_loop();
    }
}

fn wait_for_workers(state: &Arc<SharedState>) {
    let num_workers = state.workers.len();

    loop {
        let num_ready = state.ready_workers.load(Ordering::Relaxed);

        if num_ready == num_workers {
            return;
        }

        std::hint::spin_loop();
    }
}
