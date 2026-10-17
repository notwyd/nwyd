use super::*;

#[test]
fn completed_samples_scale_timestamp_ticks_without_including_submission_gaps() {
    let measured = sample(17, [1_000, 2_001_000, 6_000_000, 7_000_000], 2.0).unwrap();
    assert_eq!(measured.frame_id, 17);
    assert_eq!(measured.scene_ms, 4.0);
    assert_eq!(measured.blit_ms, 2.0);
    assert_eq!(sample(0, [5; 4], 1.0).unwrap().scene_ms, 0.0);
}

#[test]
fn invalid_or_wrapped_timestamps_are_discarded() {
    for timestamps in [
        [10, 9, 20, 30],
        [10, 20, 19, 30],
        [10, 20, 30, 29],
        [u64::MAX, 0, 1, 2],
    ] {
        assert!(sample(1, timestamps, 1.0).is_none());
    }
    for period in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        assert!(sample(1, [1, 2, 3, 4], period).is_none());
    }
}

#[test]
fn saturated_ring_skips_samples_until_a_slot_is_released() {
    let states: [Arc<AtomicU8>; SLOT_COUNT] =
        std::array::from_fn(|_| Arc::new(AtomicU8::new(IDLE)));
    for slot in 0..SLOT_COUNT {
        assert_eq!(reserve_slot(&states), Some(slot));
    }
    assert_eq!(reserve_slot(&states), None);
    for (state, value) in states.iter().zip([RESOLVED, PENDING, READY, FAILED]) {
        state.store(value, Ordering::Release);
    }
    assert_eq!(reserve_slot(&states), None);
    states[2].store(IDLE, Ordering::Release);
    assert_eq!(reserve_slot(&states), Some(2));
    assert_eq!(reserve_slot(&states), None);
}

#[cfg(not(target_arch = "wasm32"))]
fn block_on<F: std::future::Future>(future: F) -> F::Output {
    struct ThreadWake(std::thread::Thread);
    impl std::task::Wake for ThreadWake {
        fn wake(self: Arc<Self>) {
            self.0.unpark();
        }
    }
    let waker = std::task::Waker::from(Arc::new(ThreadWake(std::thread::current())));
    let mut context = std::task::Context::from_waker(&waker);
    let mut future = std::pin::pin!(future);
    loop {
        match future.as_mut().poll(&mut context) {
            std::task::Poll::Ready(result) => return result,
            std::task::Poll::Pending => std::thread::park(),
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
#[ignore = "requires a graphics adapter with timestamp queries"]
fn separate_submissions_report_asynchronously_and_recycle_slots() {
    let gpu = block_on(Gpu::open(Gpu::instance(), None)).expect("graphics adapter");
    let mut timings = GpuTimings::new(&gpu).expect("timestamp support");
    assert!(!timings.has_pending());
    let reserved: Vec<_> = (0..SLOT_COUNT)
        .map(|id| timings.begin_frame(id as u64).unwrap())
        .collect();
    assert!(timings.begin_frame(9).is_none());
    drop(reserved);

    let frame = timings.begin_frame(42).unwrap();
    let mut scene = gpu
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    assert!(!frame.resolve(&mut scene));
    frame.write(&mut scene, GpuTimingPoint::SceneStart);
    frame.write(&mut scene, GpuTimingPoint::SceneEnd);
    gpu.queue.submit([scene.finish()]);

    let mut blit = gpu
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    frame.write(&mut blit, GpuTimingPoint::BlitStart);
    frame.write(&mut blit, GpuTimingPoint::BlitEnd);
    assert!(frame.resolve(&mut blit));
    assert!(!frame.resolve(&mut blit));
    gpu.queue.submit([blit.finish()]);
    timings.finish_frame(frame);
    assert!(
        timings.has_pending()
            || timings
                .states
                .iter()
                .any(|state| state.load(Ordering::Acquire) == READY)
    );

    let reserved: Vec<_> = (0..SLOT_COUNT - 1)
        .map(|id| timings.begin_frame(id as u64 + 50).unwrap())
        .collect();
    assert!(timings.begin_frame(99).is_none());
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    let completed = loop {
        let completed = timings.poll(&gpu);
        if !completed.is_empty() {
            break completed;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "timestamp readback timed out"
        );
        std::thread::yield_now();
    };
    assert_eq!(completed.len(), 1);
    assert_eq!(completed[0].frame_id, 42);
    assert!(completed[0].scene_ms.is_finite() && completed[0].scene_ms >= 0.0);
    assert!(completed[0].blit_ms.is_finite() && completed[0].blit_ms >= 0.0);
    assert!(timings.begin_frame(99).is_some());
    assert!(timings.poll(&gpu).is_empty());
    assert!(!timings.has_pending());
    drop(reserved);
}
