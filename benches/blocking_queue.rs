//! An example of how to build a three stage pipeline


use std::fmt::{Display, Formatter};
use std::path::PathBuf;
use anyhow::Error;
use benchmark_rs::benchmarks::Benchmarks;
use benchmark_rs::stopwatch::StopWatch;
use simple_logger::SimpleLogger;
use rand::Rng;
use rand::distributions::Alphanumeric;

use command_executor::command::Command;
use command_executor::shutdown_mode::ShutdownMode;
use command_executor::thread_pool::ThreadPool;
use command_executor::thread_pool_builder::ThreadPoolBuilder;


#[derive(Clone)]
pub struct BenchmarkConfig {
    tasks: usize,
    queue_size: usize,
    commands: usize,
    string_length: usize,
    description: String,
}

impl BenchmarkConfig {
    pub fn new(tasks: usize, queue_size: usize, commands: usize, string_length: usize, description: &str) -> BenchmarkConfig {
        BenchmarkConfig {
            tasks,
            queue_size,
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

    pub fn commands(&self) -> usize {
        self.commands
    }

    pub fn string_length(&self) -> usize {
        self.string_length
    }
}

impl Display for BenchmarkConfig {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "tasks: {}, queue_size: {}, string_length: {}, description: {}",
               self.tasks,
               self.queue_size,
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
        // println!("{}", self.payload);
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

fn create_thread_pool(name: &str, tasks: usize, queue_size: usize) -> Result<ThreadPool, anyhow::Error> {
    Ok(
        ThreadPoolBuilder::new()
            .name_str(name)
            .tasks(tasks)
            .queue_size(queue_size)
            .shutdown_mode(ShutdownMode::CompletePending)
            .build()?
    )
}

fn shutdown(thread_pool: &mut ThreadPool) -> Result<(), anyhow::Error> {
    thread_pool.shutdown();
    thread_pool.join()
}

#[test]
pub fn blocking_queue_increasing_workloads() -> Result<(), anyhow::Error> {
    SimpleLogger::new().init().unwrap();
    log::info!("Started increasing workloads for blocking queue benchmark.");
    let mut benchmarks = Benchmarks::new("blocking-queue-increasing-workloads");
    let work_points: Vec<usize> = (1..=20).into_iter().map(|i| i * 1 as usize).collect();
    benchmarks.add(
        "blocking-queue-baseline",
        work0,
        BenchmarkConfig::new(
            0,
            0,
            200,
            100,
            "inline processing baseline",
        ),
        work_points.clone(),
        3,
        1,
    )?;

    benchmarks.add(
        "blocking-queue-1-tasks",
        work,
        BenchmarkConfig::new(
            1,
            4,
            200,
            20,
            "blocking queue with 1 task",
        ),
        work_points.clone(),
        3,
        1,
    )?;

    benchmarks.add(
        "blocking-queue-2-tasks",
        work,
        BenchmarkConfig::new(
            2,
            4,
            200,
            100,
            "blocking queue with 2 tasks",
        ),
        work_points.clone(),
        3,
        1,
    )?;

    benchmarks.add(
        "blocking-queue-4-tasks",
        work,
        BenchmarkConfig::new(
            4,
            4,
            200,
            100,
            "blocking queue with 4 tasks",
        ),
        work_points.clone(),
        3,
        1,
    )?;

    benchmarks.run()?;
    benchmarks.save_to_csv(PathBuf::from("./target/benchmarks/"), true, true)?;
    benchmarks.save_to_json(PathBuf::from("./target/benchmarks/"))?;

    log::info!("Finished increasing workloads for blocking queue benchmark.");
    Ok(())
}

fn work(_stop_watch: &mut StopWatch, config: BenchmarkConfig, work: usize) -> Result<(), anyhow::Error> {
    log::info!("start work: work: {}, commands: {}, tasks: {}, queue_size: {}", work, config.commands(), config.tasks(), config.queue_size());
    let mut thread_pool = create_thread_pool("trhead-pool", config.tasks(), config.queue_size())?;
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

