use engine_disruptor::{DisruptorRing, StagePipeline};
use engine_layout::RingEntry;

#[test]
fn test_disruptor_ring_publish_and_slot() {
    let mut ring = DisruptorRing::<1024>::new();
    assert_eq!(ring.capacity(), 1024);

    let mut entry = RingEntry::new();
    entry.sequence = 42;
    entry.session_id = 1001;

    let seq = ring.publish(entry);
    assert_eq!(seq, 0);

    let fetched = ring.slot(seq);
    assert_eq!(fetched.sequence, 42);
    assert_eq!(fetched.session_id, 1001);
}

#[test]
fn test_stage_pipeline_barriers() {
    let pipeline = StagePipeline::new();

    // Stage 1 progresses to 10
    pipeline.stage1_cursor.set(10);

    // Stage 2 barrier waits for stage 1
    let b2 = pipeline.barrier_for_stage2();
    let avail2 = b2.wait_for(10);
    assert!(avail2 >= 10);

    // Stage 2 progresses to 10
    pipeline.stage2_cursor.set(10);

    // Stage 3 barrier waits for stage 2
    let b3 = pipeline.barrier_for_stage3();
    let avail3 = b3.wait_for(10);
    assert!(avail3 >= 10);
}
