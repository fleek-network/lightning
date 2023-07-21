use std::{
    cell::UnsafeCell,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    thread::JoinHandle,
    time::Duration,
};

use crate::{
    message::Message,
    report::{Metrics, Report},
    state::{hook_node, with_node, NodeState},
};

const FRAME_TO_MS: u64 = 4;
const FRAME_DURATION: Duration = Duration::from_micros(1_000 / FRAME_TO_MS);

pub struct SimulationBuilder {
    executor: Box<dyn Fn() + Send + Sync>,
    num_workers: Option<usize>,
    num_nodes: Option<usize>,
    frame_per_node_report: usize,
    frame_per_global_report: usize,
}

pub struct Simulation {
    workers: Vec<JoinHandle<()>>,
    state: Arc<SharedState>,
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
    /// Store the state of each node.
    nodes: Box<[NodeState]>,
    /// The current frame.
    frame: AtomicUsize,
    /// The current node that is being processed.
    cursor: AtomicUsize,
    ready_workers: AtomicUsize,
}

// Because `SyncUnsafeCell` is unstable and nightly.
unsafe impl Sync for SharedState {}

impl SimulationBuilder {
    pub fn new<E>(executor: E) -> Self
    where
        E: Fn() + Send + Sync + 'static,
    {
        Self {
            executor: Box::new(executor),
            num_workers: None,
            num_nodes: None,
            frame_per_node_report: (FRAME_TO_MS * 16) as usize,
            frame_per_global_report: FRAME_TO_MS as usize,
        }
    }

    pub fn with_workers(mut self, n: usize) -> Self {
        assert!(n > 0, "Number of workers must be greater than 0");
        self.num_workers = Some(n);
        self
    }

    pub fn with_nodes(mut self, n: usize) -> Self {
        assert!(n > 0, "Number of nodes must be greater than 0");
        self.num_nodes = Some(n);
        self
    }

    pub fn set_node_metrics_rate(mut self, duration: Duration) -> Self {
        let rate = duration.as_nanos() / FRAME_DURATION.as_nanos();
        assert!(rate < (usize::MAX as u128));
        self.frame_per_node_report = rate as usize;
        self
    }

    pub fn set_global_metrics_rate(mut self, duration: Duration) -> Self {
        let rate = duration.as_nanos() / FRAME_DURATION.as_nanos();
        assert!(rate < (usize::MAX as u128));
        self.frame_per_global_report = rate as usize;
        self
    }

    pub fn build(self) -> Simulation {
        let num_workers = self
            .num_workers
            .unwrap_or_else(|| num_cpus::get_physical() - 1)
            .max(1);
        let num_nodes = self.num_nodes.unwrap_or(num_workers * 4);

        // Cap the number of workers to the number of nodes.
        let num_workers = num_workers.min(num_nodes);

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
            nodes: (0..num_nodes)
                .map(|i| NodeState::new(num_nodes, i))
                .collect::<Vec<_>>()
                .into_boxed_slice(),
            frame: AtomicUsize::new(0),
            cursor: AtomicUsize::new(0),
            ready_workers: AtomicUsize::new(0),
        };

        Simulation {
            workers: Vec::with_capacity(num_workers),
            state: Arc::new(state),
        }
    }
}

impl Simulation {
    pub fn run(mut self, duration: Duration) -> Self {
        self.start_threads();

        let n = duration.as_nanos() / FRAME_DURATION.as_nanos();
        for _ in 0..n {
            wait_for_workers(&self.state);
            self.state.ready_workers.store(0, Ordering::Relaxed);
            self.state.cursor.store(0, Ordering::Relaxed);
            self.state.frame.fetch_add(1, Ordering::Relaxed);
            self.run_post_frame();
        }

        wait_for_workers(&self.state);
        self.run_post_frame();

        self.stop_threads();

        self
    }

    pub fn finish(self) -> Report {
        let mut report = self
            .state
            .workers
            .iter()
            .map(|v| unsafe { &mut *v.get() })
            .map(|s| std::mem::take(&mut s.metrics))
            .fold(Report::default(), |a, b| a + b);

        let count = self.state.nodes.len();
        let nodes = self.state.nodes.as_ptr();

        for i in 0..count {
            let node = unsafe { &mut *(nodes.add(i) as *mut NodeState) };
            report.node.push(std::mem::take(&mut node.metrics));
        }

        report
    }

    fn run_post_frame(&mut self) {
        // Move the messages generated by each worker to each of the destinations.
        for messages in self
            .state
            .workers
            .iter()
            .map(|s| &mut unsafe { &mut *s.get() }.outgoing)
        {
            for mut msg in messages.drain(..) {
                let node_id = msg.receiver.0;
                let node_ref =
                    unsafe { &mut *(self.state.nodes.as_ptr().add(node_id) as *mut NodeState) };

                msg.time += self.get_latency(msg.sender.0, msg.receiver.0).as_nanos();

                node_ref.received.insert(msg);
            }
        }
    }

    fn get_latency(&self, _s: usize, _r: usize) -> Duration {
        // todo
        Duration::from_millis(1)
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
        let frame = self.state.frame.load(Ordering::Relaxed);
        self.state.frame.store(usize::MAX, Ordering::Relaxed);

        while let Some(handle) = self.workers.pop() {
            handle.join().expect("Worker thread paniced.");
        }

        self.state.frame.store(frame, Ordering::Relaxed);
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
        if wait_for_next_frame(&state, current_frame) {
            break;
        }

        if state.frame_per_global_report > 0 && current_frame % state.frame_per_global_report == 0 {
            worker_state.metrics.next_period();
        }

        loop {
            let index = state.cursor.fetch_add(1, Ordering::Relaxed);

            if index >= state.nodes.len() {
                break;
            }

            execute_node(&state, worker_state, current_frame, index);
            hook_node(std::ptr::null_mut());
        }

        current_frame += 1;
    }
}

fn execute_node(
    state: &Arc<SharedState>,
    worker_state: &mut WorkerState,
    frame: usize,
    index: usize,
) {
    let ptr = unsafe { state.nodes.as_ptr().add(index) as *mut NodeState };
    hook_node(ptr);

    // update the time on the node.
    with_node(|n| {
        debug_assert_eq!(n.node_id, index);
        n.time = (frame as u128) * FRAME_DURATION.as_nanos();

        if state.frame_per_node_report > 0 && frame % state.frame_per_node_report == 0 {
            n.metrics.next_period();
        }
    });

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
        n.metrics.insert(n.current_metrics);
        worker_state.metrics.insert(n.current_metrics);
        n.current_metrics = Metrics::default();
    });
}

fn wait_for_next_frame(state: &Arc<SharedState>, current_frame: usize) -> bool {
    loop {
        let frame = state.frame.load(Ordering::Relaxed);

        if frame == usize::MAX {
            return true;
        }

        if frame == current_frame + 1 {
            return false;
        }

        debug_assert!(frame == current_frame, "Frame was skipped.");

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

#[cfg(test)]
mod test {
    use std::time::Duration;

    use crate::{api, simulation::SimulationBuilder};

    #[test]
    fn x() {
        let report = SimulationBuilder::new(exec)
            .with_nodes(2)
            .build()
            .run(Duration::from_secs(1))
            .finish();

        println!("{report:#?}");
    }

    fn exec() {
        api::spawn(async {
            println!("Spawn from {:?}", api::RemoteAddr::whoami());

            if api::RemoteAddr::whoami().0 == 0 {
                let mut listener = api::listen(18);

                while let Some(mut conn) = listener.accept().await {
                    println!("Connection accepted from {:?}", conn.remote());
                    conn.write(&18);
                }
            } else {
                let res = api::connect(api::RemoteAddr(0), 18).await;
                println!("connect result = {:?}", res.is_ok());
                let mut conn = res.unwrap();
                let data = conn.recv::<i32>().await;
                println!("Data received {data:?}");
            }
        });

        println!("Hello! {:?}", api::RemoteAddr::whoami());
    }
}
