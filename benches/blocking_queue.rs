//! An example of how to build a three stage pipeline

use std::fmt::{Display, Formatter};
use std::path::PathBuf;

use anyhow::Error;
use benchmark_rs::benchmarks::Benchmarks;
use benchmark_rs::stopwatch::StopWatch;
use rand::distributions::Alphanumeric;
use rand::Rng;
use simple_logger::SimpleLogger;

use command_executor::command::Command;
use command_executor::queue_type::QueueType;
use command_executor::shutdown_mode::ShutdownMode;
use command_executor::thread_pool::ThreadPool;
use command_executor::thread_pool_builder::ThreadPoolBuilder;

#[derive(Clone)]
pub struct BenchmarkConfig {
    tasks: usize,
    queue_size: usize,
    queue_type: QueueType,
    commands: usize,
    string_length: usize,
    description: String,
}

impl BenchmarkConfig {
    pub fn new(tasks: usize, queue_size: usize, queue_type: QueueType, commands: usize, string_length: usize, description: &str) -> BenchmarkConfig {
        BenchmarkConfig {
            tasks,
            queue_size,
            queue_type,
            commands,
            string_length,
            description: description.to_string(),
        }
    }

    pub fn tasks(&self) -> usize {
        self.tasks
    }

    pub fn queue_size(&self) -> usize {
        self.queue_size
    }

    pub fn queue_type(&self) -> QueueType {
        self.queue_type
    }

    pub fn commands(&self) -> usize {
        self.commands
    }

    pub fn string_length(&self) -> usize {
        self.string_length
    }
}

impl Display for BenchmarkConfig {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "tasks: {}, queue_size: {}, queue_type: {}, string_length: {}, description: {}",
               self.tasks,
               self.queue_size,
               self.queue_type,
               self.string_length,
               self.description,
        )
    }
}

struct ProcessingCommand {
    count: usize,
    length: usize,
}

impl ProcessingCommand {
    fn new(count: usize, length: usize) -> ProcessingCommand {
        ProcessingCommand {
            count,
            length,
        }
    }
}

impl Command for ProcessingCommand {
    fn execute(&self) -> Result<(), Error> {
        let mut v: Vec<String> = Vec::with_capacity(self.count);
        for _i in 0..self.count {
            v.push(
                rand::thread_rng()
                    .sample_iter(&Alphanumeric)
                    .take(self.length)
                    .map(char::from)
                    .collect::<String>()
            );
        }
        v.sort();
        Ok(())
    }
}

fn create_thread_pool(name: &str, tasks: usize, queue_size: usize, queue_type: QueueType) -> Result<ThreadPool, anyhow::Error> {
    Ok(
        ThreadPoolBuilder::new()
            .with_name_str(name)
            .with_tasks(tasks)
            .with_queue_type(queue_type)
            .with_queue_size(queue_size)
            .with_shutdown_mode(ShutdownMode::CompletePending)
            .build()?
    )
}

fn shutdown(thread_pool: &mut ThreadPool) -> Result<(), anyhow::Error> {
    thread_pool.shutdown();
    thread_pool.join()
}

#[test]
fn queue_types_bench() -> Result<(), Error> {
    SimpleLogger::new().init().unwrap();
    log::info!("Started increasing workloads benchmark.");
    for i in 0..5 {
        increasing_workloads(10_usize.pow(i), 4, 200, 25)?
    }
    log::info!("Finished increasing workloads benchmark.");
    Ok(())
}

fn bench_name(base: &str, magnitude: usize) -> String {
    format!("{}-magnitude-{}", base, magnitude)
}

fn increasing_workloads(magnitude: usize, queue_size: usize, commands: usize, string_length: usize) -> Result<(), Error> {
    let mut benchmarks = Benchmarks::new(bench_name("blocking-queue-increasing-workloads", magnitude).as_str());
    let work_points: Vec<usize> = (1..=20).into_iter().map(|i| i * magnitude).collect();
    benchmarks.add(
        bench_name("blocking-queue-baseline", magnitude).as_str(),
        work0,
        BenchmarkConfig::new(
            0,
            0,
            QueueType::BlockingQueue,
            commands,
            string_length,
            "inline processing baseline",
        ),
        work_points.clone(),
        3,
        1,
    )?;

    benchmarks.add(
        bench_name("blocking-queue-1-tasks", magnitude).as_str(),
        work,
        BenchmarkConfig::new(
            1,
            queue_size,
            QueueType::BlockingQueue,
            commands,
            string_length,
            "",
        ),
        work_points.clone(),
        3,
        1,
    )?;

    benchmarks.add(
        bench_name("crossbeam-blocking-queue-1-tasks", magnitude).as_str(),
        work,
        BenchmarkConfig::new(
            1,
            queue_size,
            QueueType::CrossbeamBlockingQueue,
            commands,
            string_length,
            "",
        ),
        work_points.clone(),
        3,
        1,
    )?;

    benchmarks.add(
        bench_name("blocking-queue-2-tasks", magnitude).as_str(),
        work,
        BenchmarkConfig::new(
            2,
            queue_size,
            QueueType::BlockingQueue,
            commands,
            string_length,
            "",
        ),
        work_points.clone(),
        3,
        1,
    )?;

    benchmarks.add(
        bench_name("crossbeam-blocking-queue-2-tasks", magnitude).as_str(),
        work,
        BenchmarkConfig::new(
            2,
            queue_size,
            QueueType::CrossbeamBlockingQueue,
            commands,
            string_length,
            "",
        ),
        work_points.clone(),
        3,
        1,
    )?;

    benchmarks.add(
        bench_name("blocking-queue-4-tasks", magnitude).as_str(),
        work,
        BenchmarkConfig::new(
            4,
            queue_size,
            QueueType::BlockingQueue,
            commands,
            string_length,
            "",
        ),
        work_points.clone(),
        3,
        1,
    )?;

    benchmarks.add(
        bench_name("crossbeam-blocking-queue-4-tasks", magnitude).as_str(),
        work,
        BenchmarkConfig::new(
            4,
            queue_size,
            QueueType::CrossbeamBlockingQueue,
            commands,
            string_length,
            "",
        ),
        work_points.clone(),
        3,
        1,
    )?;

    benchmarks.run()?;
    benchmarks.save_to_csv(PathBuf::from("./target/benchmarks/"), true, true)?;
    benchmarks.save_to_json(PathBuf::from("./target/benchmarks/"))?;

    Ok(())
}

fn work(_stop_watch: &mut StopWatch, config: BenchmarkConfig, work: usize) -> Result<(), anyhow::Error> {
    log::info!("start work: work: {}, commands: {}, tasks: {}, queue_size: {}", work, config.commands(), config.tasks(), config.queue_size());
    let mut thread_pool = create_thread_pool(
        "trhead-pool",
        config.tasks(),
        config.queue_size(),
        config.queue_type(),
    )?;
    for _i in 0..config.commands() {
        thread_pool.submit(Box::new(ProcessingCommand::new(work, config.string_length())));
    }
    shutdown(&mut thread_pool)?;
    log::info!("finish work");
    Ok(())
}

fn work0(_stop_watch: &mut StopWatch, config: BenchmarkConfig, work: usize) -> Result<(), Error> {
    log::info!("start work0: work: {}, commands: {}, tasks: {}", work, config.commands(), config.tasks());
    for _i in 0..config.commands() {
        let mut v: Vec<String> = Vec::with_capacity(work);
        for _i in 0..work {
            v.push(
                rand::thread_rng()
                    .sample_iter(&Alphanumeric)
                    .take(config.string_length())
                    .map(char::from)
                    .collect::<String>()
            );
        }
        v.sort();
    }
    log::info!("finish work0");
    Ok(())
}

