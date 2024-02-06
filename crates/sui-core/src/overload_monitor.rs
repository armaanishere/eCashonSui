// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

use crate::authority::AuthorityState;
use std::cmp::{max, min};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Weak;
use std::time::Duration;
use sui_config::node::OverloadThresholdConfig;
use tokio::time::sleep;
use tracing::{debug, info};

#[derive(Default)]
pub struct AuthorityOverloadInfo {
    /// Whether the authority is overloaded.
    pub is_overload: AtomicBool,

    /// The calculated percentage of transactions to drop.
    pub load_shedding_percentage: AtomicU32,
}

impl AuthorityOverloadInfo {
    pub fn set_overload(&self, load_shedding_percentage: u32) {
        self.is_overload.store(true, Ordering::Relaxed);
        self.load_shedding_percentage
            .store(min(load_shedding_percentage, 100), Ordering::Relaxed);
    }

    pub fn clear_overload(&self) {
        self.is_overload.store(false, Ordering::Relaxed);
        self.load_shedding_percentage.store(0, Ordering::Relaxed);
    }
}

// Monitors the overload signals in `authority_state` periodically, and updates its `overload_info`
// when the signals indicates overload.
pub async fn overload_monitor(
    authority_state: Weak<AuthorityState>,
    config: OverloadThresholdConfig,
) {
    info!("Starting system overload monitor.");

    loop {
        let authority_exist = check_authority_overload(&authority_state, &config);
        if !authority_exist {
            // `authority_state` doesn't exist anymore. Quit overload monitor.
            break;
        }
        sleep(config.overload_monitor_interval).await;
    }

    info!("Shut down system overload monitor.");
}

// Checks authority overload signals, and updates authority's `overload_info`.
// Returns whether the authority state exists.
fn check_authority_overload(
    authority_state: &Weak<AuthorityState>,
    config: &OverloadThresholdConfig,
) -> bool {
    let authority_arc = authority_state.upgrade();
    if authority_arc.is_none() {
        // `authority_state` doesn't exist anymore.
        return false;
    }

    let authority = authority_arc.unwrap();
    let queueing_latency = authority
        .metrics
        .execution_queueing_latency
        .latency()
        .unwrap_or_default();
    let txn_ready_rate = authority.metrics.txn_ready_rate_tracker.lock().rate();
    let execution_rate = authority.metrics.execution_rate_tracker.lock().rate();

    debug!(
        "Check authority overload signal, queueing latency {:?}, ready rate {:?}, execution rate {:?}.",
        queueing_latency, txn_ready_rate, execution_rate
    );

    let (is_overload, load_shedding_percentage) = check_overload_signals(
        config,
        authority
            .overload_info
            .load_shedding_percentage
            .load(Ordering::Relaxed),
        queueing_latency,
        txn_ready_rate,
        execution_rate,
    );

    if is_overload {
        authority
            .overload_info
            .set_overload(load_shedding_percentage);
    } else {
        authority.overload_info.clear_overload();
    }

    authority
        .metrics
        .authority_overload_status
        .set(is_overload as i64);
    authority
        .metrics
        .authority_load_shedding_percentage
        .set(load_shedding_percentage as i64);
    true
}

// Calculates the percentage of transactions to drop in order to reduce execution queue.
// Returns the integer percentage between 0 and 100.
fn calculate_load_shedding_percentage(txn_ready_rate: f64, execution_rate: f64) -> u32 {
    // When transaction ready rate is practically 0, we aren't adding more load to the
    // execution driver, so no shedding.
    // TODO: consensus handler or transaction manager can also be overloaded.
    if txn_ready_rate < 1e-10 {
        return 0;
    }

    // Deflate the execution rate to account for the case that execution_rate is close to
    // txn_ready_rate.
    if execution_rate * 0.9 > txn_ready_rate {
        return 0;
    }

    // In order to maintain execution queue length, we need to drop at least (1 - executionRate / readyRate).
    // To reduce the queue length, here we add 10% more transactions to drop.
    (((1.0 - execution_rate * 0.9 / txn_ready_rate) + 0.1).min(1.0) * 100.0).round() as u32
}

// Given overload signals (`queueing_latency`, `txn_ready_rate`, `execution_rate`), return whether
// the authority server should enter load shedding mode, and how much percentage of transactions to drop.
fn check_overload_signals(
    config: &OverloadThresholdConfig,
    current_load_shedding_percentage: u32,
    queueing_latency: Duration,
    txn_ready_rate: f64,
    execution_rate: f64,
) -> (bool, u32) {
    let additional_load_shedding_percentage;
    if queueing_latency > config.execution_queue_latency_hard_limit {
        let calculated_load_shedding_percentage =
            calculate_load_shedding_percentage(txn_ready_rate, execution_rate);

        additional_load_shedding_percentage = if calculated_load_shedding_percentage > 0
            || txn_ready_rate >= config.safe_transaction_ready_rate as f64
        {
            max(
                calculated_load_shedding_percentage,
                config.min_load_shedding_percentage_above_hard_limit,
            )
        } else {
            0
        };
    } else if queueing_latency > config.execution_queue_latency_soft_limit {
        additional_load_shedding_percentage =
            calculate_load_shedding_percentage(txn_ready_rate, execution_rate);
    } else {
        additional_load_shedding_percentage = 0;
    }

    let load_shedding_percentage = if additional_load_shedding_percentage > 0 {
        current_load_shedding_percentage
            + (100 - current_load_shedding_percentage) * additional_load_shedding_percentage / 100
    } else {
        if txn_ready_rate > config.safe_transaction_ready_rate as f64
            && current_load_shedding_percentage > 10
        {
            current_load_shedding_percentage - 10
        } else {
            0
        }
    };

    let load_shedding_percentage = min(
        load_shedding_percentage,
        config.max_load_shedding_percentage,
    );
    let overload_status = load_shedding_percentage > 0;
    (overload_status, load_shedding_percentage)
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::authority::test_authority_builder::TestAuthorityBuilder;
    use rand::{
        rngs::{OsRng, StdRng},
        Rng, SeedableRng,
    };
    use std::sync::Arc;
    use tokio::sync::mpsc::unbounded_channel;
    use tokio::sync::mpsc::UnboundedReceiver;
    use tokio::sync::mpsc::UnboundedSender;
    use tokio::sync::oneshot;
    use tokio::task::JoinHandle;
    use tokio::time::{interval, pause, resume, Instant, MissedTickBehavior};

    #[test]
    pub fn test_authority_overload_info() {
        let overload_info = AuthorityOverloadInfo::default();
        assert!(!overload_info.is_overload.load(Ordering::Relaxed));
        assert_eq!(
            overload_info
                .load_shedding_percentage
                .load(Ordering::Relaxed),
            0
        );

        {
            overload_info.set_overload(20);
            assert!(overload_info.is_overload.load(Ordering::Relaxed));
            assert_eq!(
                overload_info
                    .load_shedding_percentage
                    .load(Ordering::Relaxed),
                20
            );
        }

        // Tests that load shedding percentage can't go beyond 100%.
        {
            overload_info.set_overload(110);
            assert!(overload_info.is_overload.load(Ordering::Relaxed));
            assert_eq!(
                overload_info
                    .load_shedding_percentage
                    .load(Ordering::Relaxed),
                100
            );
        }

        {
            overload_info.clear_overload();
            assert!(!overload_info.is_overload.load(Ordering::Relaxed));
            assert_eq!(
                overload_info
                    .load_shedding_percentage
                    .load(Ordering::Relaxed),
                0
            );
        }
    }

    #[test]
    pub fn test_calculate_load_shedding_ratio() {
        assert_eq!(calculate_load_shedding_percentage(90.0, 100.1), 0);
        assert_eq!(calculate_load_shedding_percentage(90.0, 100.0), 10);
        assert_eq!(calculate_load_shedding_percentage(100.0, 100.0), 20);
        assert_eq!(calculate_load_shedding_percentage(110.0, 100.0), 28);
        assert_eq!(calculate_load_shedding_percentage(180.0, 100.0), 60);
        assert_eq!(calculate_load_shedding_percentage(100.0, 0.0), 100);
        assert_eq!(calculate_load_shedding_percentage(0.0, 1.0), 0);
    }

    #[test]
    pub fn test_check_overload_signals() {
        let config = OverloadThresholdConfig {
            execution_queue_latency_hard_limit: Duration::from_secs(10),
            execution_queue_latency_soft_limit: Duration::from_secs(1),
            max_load_shedding_percentage: 90,
            ..Default::default()
        };

        // When execution queueing latency is within soft limit, don't start overload protection.
        assert_eq!(
            check_overload_signals(&config, 0, Duration::from_millis(500), 1000.0, 10.0),
            (false, 0)
        );

        // When execution queueing latency hits soft limit and execution rate is higher, don't
        // start overload protection.
        assert_eq!(
            check_overload_signals(&config, 0, Duration::from_secs(2), 100.0, 120.0),
            (false, 0)
        );

        // When execution queueing latency hits soft limit, but not hard limit, start overload
        // protection.
        assert_eq!(
            check_overload_signals(&config, 0, Duration::from_secs(2), 100.0, 100.0),
            (true, 20)
        );

        // When execution queueing latency hits hard limit, start more aggressive overload
        // protection.
        assert_eq!(
            check_overload_signals(&config, 0, Duration::from_secs(11), 100.0, 100.0),
            (true, 50)
        );

        // When execution queueing latency hits hard limit and calculated shedding percentage
        // is higher than min_load_shedding_percentage_above_hard_limit.
        assert_eq!(
            check_overload_signals(&config, 0, Duration::from_secs(11), 240.0, 100.0),
            (true, 73)
        );

        // When execution queueing latency hits hard limit, but transaction ready rate
        // is within safe_transaction_ready_rate, don't start overload protection.
        assert_eq!(
            check_overload_signals(&config, 0, Duration::from_secs(11), 20.0, 100.0),
            (false, 0)
        );

        // Maximum transactions shed is cap by `max_load_shedding_percentage` config.
        assert_eq!(
            check_overload_signals(&config, 0, Duration::from_secs(11), 100.0, 0.0),
            (true, 90)
        );

        assert_eq!(
            check_overload_signals(&config, 50, Duration::from_secs(2), 100.0, 100.0),
            (true, 60)
        );

        assert_eq!(
            check_overload_signals(&config, 90, Duration::from_secs(2), 200.0, 300.0),
            (true, 80)
        );

        assert_eq!(
            check_overload_signals(&config, 50, Duration::from_secs(11), 100.0, 100.0),
            (true, 75)
        );
    }

    #[tokio::test(flavor = "current_thread")]
    pub async fn test_check_authority_overload() {
        let config = OverloadThresholdConfig {
            safe_transaction_ready_rate: 0,
            ..Default::default()
        };
        let state = TestAuthorityBuilder::new()
            .with_overload_threshold_config(config.clone())
            .build()
            .await;

        // Creates a simple case to see if authority state overload_info can be updated
        // correctly by check_authority_overload.
        state
            .metrics
            .execution_queueing_latency
            .report(Duration::from_secs(20));
        let authority = Arc::downgrade(&state);
        assert!(check_authority_overload(&authority, &config));
        assert!(state.overload_info.is_overload.load(Ordering::Relaxed));
        assert_eq!(
            state
                .overload_info
                .load_shedding_percentage
                .load(Ordering::Relaxed),
            config.min_load_shedding_percentage_above_hard_limit
        );

        // Checks that check_authority_overload should return false when the input
        // authority state doesn't exist.
        let authority = Arc::downgrade(&state);
        drop(state);
        assert!(!check_authority_overload(&authority, &config));
    }

    fn start_load_generator(
        steady_rate: f64,
        tx: UnboundedSender<Instant>,
        mut burst_rx: UnboundedReceiver<u32>,
        authority: Arc<AuthorityState>,
        enable_load_shedding: bool,
        total_requests_arc: Arc<AtomicU32>,
        dropped_requests_arc: Arc<AtomicU32>,
    ) -> JoinHandle<()> {
        tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs_f64(1.0 / steady_rate));
            let mut rng = StdRng::from_rng(&mut OsRng).unwrap();
            let mut total_requests: u32 = 0;
            let mut total_dropped_requests: u32 = 0;

            // Helper function to check whether we should send a request.
            let mut do_send =
                |enable_load_shedding: bool, authority: Arc<AuthorityState>| -> bool {
                    if enable_load_shedding {
                        let shedding_percentage = authority
                            .overload_info
                            .load_shedding_percentage
                            .load(Ordering::Relaxed);
                        if shedding_percentage > 0 && rng.gen_range(0..100) < shedding_percentage {
                            false
                        } else {
                            true
                        }
                    } else {
                        true
                    }
                };

            loop {
                tokio::select! {
                    now = interval.tick() => {
                        total_requests += 1;
                        if do_send(enable_load_shedding, authority.clone()) {
                            if tx.send(now).is_err() {
                                info!("Load generator stopping. Total requests {:?}, total dropped requests {:?}.", total_requests, total_dropped_requests);
                                total_requests_arc.store(total_requests, Ordering::SeqCst);
                                dropped_requests_arc.store(total_dropped_requests, Ordering::SeqCst);
                                return;
                            }
                            authority.metrics.txn_ready_rate_tracker.lock().record();
                        } else {
                            total_dropped_requests += 1;
                        }
                    }
                    Some(burst) = burst_rx.recv() => {
                        let now = Instant::now();
                        total_requests += burst;
                        for _ in 0..burst {
                            if do_send(enable_load_shedding, authority.clone()) {
                                if tx.send(now).is_err() {
                                    info!("Load generator stopping. Total requests {:?}, total dropped requests {:?}.", total_requests, total_dropped_requests);
                                    total_requests_arc.store(total_requests, Ordering::SeqCst);
                                    dropped_requests_arc.store(total_dropped_requests, Ordering::SeqCst);
                                    return;
                                }
                                authority.metrics.txn_ready_rate_tracker.lock().record();
                            } else {
                                total_dropped_requests += 1;
                            }
                        }
                    }
                }
            }
        })
    }

    fn start_executor(
        execution_rate: f64,
        mut rx: UnboundedReceiver<Instant>,
        mut stop_rx: oneshot::Receiver<()>,
        authority: Arc<AuthorityState>,
    ) -> JoinHandle<()> {
        tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs_f64(1.0 / execution_rate));
            interval.set_missed_tick_behavior(MissedTickBehavior::Skip);
            loop {
                tokio::select! {
                    Some(start_time) = rx.recv() => {
                        authority.metrics.execution_rate_tracker.lock().record();
                        authority.metrics.execution_queueing_latency.report(start_time.elapsed());
                        interval.tick().await;
                    }
                    _ = &mut stop_rx => {
                        // Stop signal received
                        info!("Executor stopping");
                        return;
                    }
                }
            }
        })
    }

    async fn sleep_and_print_stats(state: Arc<AuthorityState>, seconds: u32) {
        for _ in 0..seconds {
            info!(
                "Overload: {:?}. Shedding percentage: {:?}. Queue: {:?}, Ready rate: {:?}. Exec rate: {:?}.",
                state.overload_info.is_overload.load(Ordering::Relaxed),
                state
                    .overload_info
                    .load_shedding_percentage
                    .load(Ordering::Relaxed),
                state.metrics.execution_queueing_latency.latency(),
                state.metrics.txn_ready_rate_tracker.lock().rate(),
                state.metrics.execution_rate_tracker.lock().rate(),
            );
            sleep(Duration::from_secs(1)).await;
        }
    }

    async fn run_consistent_workload_test(
        generator_rate: f64,
        executor_rate: f64,
        min_dropping_rate: f64,
        max_dropping_rate: f64,
    ) {
        let state = TestAuthorityBuilder::new().build().await;

        let (tx, rx) = unbounded_channel();
        let (_burst_tx, burst_rx) = unbounded_channel();
        let total_requests = Arc::new(AtomicU32::new(0));
        let dropped_requests = Arc::new(AtomicU32::new(0));
        let load_generator = start_load_generator(
            generator_rate,
            tx.clone(),
            burst_rx,
            state.clone(),
            true,
            total_requests.clone(),
            dropped_requests.clone(),
        );

        let (stop_tx, stop_rx) = oneshot::channel();
        let executor = start_executor(executor_rate, rx, stop_rx, state.clone());

        sleep_and_print_stats(state.clone(), 50).await;

        stop_tx.send(()).unwrap();
        let _ = tokio::join!(load_generator, executor);

        let dropped_ratio = dropped_requests.load(Ordering::SeqCst) as f64
            / total_requests.load(Ordering::SeqCst) as f64;
        assert!(min_dropping_rate < dropped_ratio);
        assert!(dropped_ratio < max_dropping_rate);
    }

    #[tokio::test(flavor = "multi_thread")]
    pub async fn test_workload_consistent_slightly_overload() {
        telemetry_subscribers::init_for_testing();
        run_consistent_workload_test(1100.0, 1000.0, 0.05, 0.25).await;
    }

    #[tokio::test(flavor = "multi_thread")]
    pub async fn test_workload_consistent_overload() {
        telemetry_subscribers::init_for_testing();
        run_consistent_workload_test(3000.0, 1000.0, 0.6, 0.8).await;
    }

    #[tokio::test(flavor = "current_thread")]
    pub async fn test_workload_single_spike() {
        telemetry_subscribers::init_for_testing();
        let state = TestAuthorityBuilder::new().build().await;

        let (tx, rx) = unbounded_channel();
        let (burst_tx, burst_rx) = unbounded_channel();
        let total_requests = Arc::new(AtomicU32::new(0));
        let dropped_requests = Arc::new(AtomicU32::new(0));
        let load_generator = start_load_generator(
            10.0,
            tx.clone(),
            burst_rx,
            state.clone(),
            true,
            total_requests.clone(),
            dropped_requests.clone(),
        );

        let (stop_tx, stop_rx) = oneshot::channel();
        let executor = start_executor(1000.0, rx, stop_rx, state.clone());

        sleep_and_print_stats(state.clone(), 10).await;
        pause();
        burst_tx.send(8000).unwrap();
        resume();
        sleep_and_print_stats(state.clone(), 20).await;

        stop_tx.send(()).unwrap();
        let _ = tokio::join!(load_generator, executor);
        assert_eq!(dropped_requests.load(Ordering::SeqCst), 0);
    }

    #[tokio::test(flavor = "current_thread")]
    pub async fn test_workload_consistent_short_spike() {
        telemetry_subscribers::init_for_testing();
        let state = TestAuthorityBuilder::new().build().await;

        let (tx, rx) = unbounded_channel();
        let (burst_tx, burst_rx) = unbounded_channel();
        let total_requests = Arc::new(AtomicU32::new(0));
        let dropped_requests = Arc::new(AtomicU32::new(0));
        let load_generator = start_load_generator(
            10.0,
            tx.clone(),
            burst_rx,
            state.clone(),
            true,
            total_requests.clone(),
            dropped_requests.clone(),
        );

        let (stop_tx, stop_rx) = oneshot::channel();
        let executor = start_executor(1000.0, rx, stop_rx, state.clone());

        sleep_and_print_stats(state.clone(), 15).await;
        for _ in 0..8 {
            pause();
            burst_tx.send(10000).unwrap();
            resume();
            sleep_and_print_stats(state.clone(), 5).await;
        }

        stop_tx.send(()).unwrap();
        let _ = tokio::join!(load_generator, executor);
        let dropped_ratio = dropped_requests.load(Ordering::SeqCst) as f64
            / total_requests.load(Ordering::SeqCst) as f64;
        assert!(0.4 < dropped_ratio);
        assert!(dropped_ratio < 0.6);
    }
}
