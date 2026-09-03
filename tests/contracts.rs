use std::{
    fmt::Debug,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread,
};

use schedlib::durable::{
    Checkpoint, DurableOutcome, PlanIdentity, ProtocolInput, ProtocolReport, ResumeMachine,
};
use schedlib_interop::{
    decode_checkpoint_for, decode_checkpoint_with_control, decode_plan, decode_plan_with_control,
    decode_verified_checkpoint_for, decode_verified_plan, digest_checkpoint, digest_plan,
    encode_checkpoint, encode_checkpoint_with_control, encode_plan, encode_plan_with_control,
    CanonicalKeyCodec, CodecControl, CodecLimits, InteropError, KeyCodecError, KeyCodecId,
    U64KeyCodec, CHECKPOINT_HEADER_BYTES, CHECKPOINT_MAGIC, CHECKPOINT_SCHEMA_ID,
    CHECKPOINT_VERSION, DIGEST_CHECKPOINT_CONTEXT, DIGEST_PLAN_CONTEXT, PLAN_HEADER_BYTES,
    PLAN_MAGIC, PLAN_SCHEMA_ID, PLAN_VERSION,
};

type Outcome = DurableOutcome<u64, u64, u64>;
type Report = ProtocolReport<u64, u64, u64, u64>;

fn must_ok<T, E: Debug>(result: Result<T, E>) -> T {
    match result {
        Ok(value) => value,
        Err(error) => panic!("expected success, received {error:?}"),
    }
}

fn must_err<T, E: Debug>(result: Result<T, E>) -> E {
    match result {
        Ok(_) => panic!("expected rejection"),
        Err(error) => error,
    }
}

fn plan(task_count: usize) -> PlanIdentity<u64, String> {
    let keys: Vec<_> = (0..task_count as u64).collect();
    let dependencies = (1..task_count as u64)
        .map(|target| (target - 1, target))
        .collect();
    let effects = keys
        .iter()
        .map(|key| {
            (
                vec![key.saturating_mul(2)],
                vec![key.saturating_mul(2).saturating_add(1)],
            )
        })
        .collect();
    let costs = keys.iter().map(|key| key.saturating_add(1)).collect();
    must_ok(PlanIdentity::new(
        7,
        keys,
        dependencies,
        effects,
        costs,
        u64::MAX,
        "schedlib-interop-tests".to_owned(),
    ))
}

fn plan_with_keys(keys: Vec<u64>) -> PlanIdentity<u64, String> {
    let count = keys.len();
    must_ok(PlanIdentity::new(
        7,
        keys,
        vec![],
        vec![(vec![], vec![]); count],
        vec![1; count],
        9,
        String::new(),
    ))
}

fn report(task_count: usize) -> Report {
    let outcomes = (0..task_count as u64).map(Outcome::Success).collect();
    must_ok(ResumeMachine::run(ProtocolInput::new(
        plan(task_count),
        outcomes,
    )))
}

fn checkpoint(task_count: usize, prefix: usize) -> Checkpoint<u64> {
    must_ok(Checkpoint::from_committed_prefix(
        &report(task_count),
        prefix,
    ))
}

fn encode_sample_plan() -> Vec<u8> {
    must_ok(encode_plan(
        &plan(3),
        &U64KeyCodec,
        CodecLimits::unbounded(),
    ))
}

fn encode_sample_checkpoint() -> Vec<u8> {
    must_ok(encode_checkpoint(
        &checkpoint(3, 2),
        &U64KeyCodec,
        CodecLimits::unbounded(),
    ))
}

fn replace_u64(bytes: &mut [u8], offset: usize, value: u64) {
    bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}

#[test]
fn prop_machine_state_is_typed() {
    let encoded = encode_sample_plan();
    let decoded = must_ok(decode_plan_with_control(
        &encoded,
        &U64KeyCodec,
        CodecLimits::unbounded(),
        CodecControl::unlimited(),
    ));
    assert!(decoded.metrics().published());
    assert!(decoded.metrics().cursor() <= encoded.len() as u64);
}

#[test]
fn prop_wire_words_are_little_endian() {
    let encoded = encode_sample_plan();
    assert_eq!(&encoded[64..72], &7_u64.to_le_bytes());
    assert_eq!(
        u16::from_le_bytes([encoded[24], encoded[25]]),
        PLAN_VERSION.0
    );
}

#[test]
fn prop_wire_uses_no_usize_layout() {
    assert_eq!(PLAN_HEADER_BYTES, 112);
    assert_eq!(CHECKPOINT_HEADER_BYTES, 96);
    assert_eq!(PLAN_VERSION, (1, 0));
    assert_eq!(CHECKPOINT_VERSION, (1, 0));
}

#[test]
fn prop_header_fields_match_exactly() {
    let plan_bytes = encode_sample_plan();
    assert_eq!(&plan_bytes[..8], &PLAN_MAGIC);
    assert_eq!(&plan_bytes[8..24], &PLAN_SCHEMA_ID);
    for offset in [0, 8, 24, 26, 28, 32] {
        let mut changed = plan_bytes.clone();
        changed[offset] ^= 1;
        assert!(decode_plan(&changed, &U64KeyCodec, CodecLimits::unbounded()).is_err());
    }
    let checkpoint_bytes = encode_sample_checkpoint();
    assert_eq!(&checkpoint_bytes[..8], &CHECKPOINT_MAGIC);
    assert_eq!(&checkpoint_bytes[8..24], &CHECKPOINT_SCHEMA_ID);
}

#[test]
fn prop_declared_and_actual_lengths_match() {
    let encoded = encode_sample_plan();
    let mut trailing = encoded.clone();
    trailing.push(0);
    assert!(matches!(
        must_err(decode_plan(
            &trailing,
            &U64KeyCodec,
            CodecLimits::unbounded()
        )),
        InteropError::LengthMismatch { .. }
    ));
    for end in 0..encoded.len() {
        assert!(decode_plan(&encoded[..end], &U64KeyCodec, CodecLimits::unbounded()).is_err());
    }
}

#[test]
fn prop_length_arithmetic_never_wraps() {
    let mut encoded = encode_sample_plan();
    replace_u64(&mut encoded, 104, u64::MAX);
    assert!(matches!(
        must_err(decode_plan(
            &encoded,
            &U64KeyCodec,
            CodecLimits::unbounded()
        )),
        InteropError::ArithmeticOverflow | InteropError::LengthMismatch { .. }
    ));
}

#[test]
fn prop_byte_limit_precedes_allocation() {
    let encoded = encode_sample_plan();
    let limits = CodecLimits {
        max_bytes: encoded.len() as u64 - 1,
        ..CodecLimits::unbounded()
    };
    assert!(matches!(
        must_err(decode_plan(&encoded, &U64KeyCodec, limits)),
        InteropError::ByteLimitExceeded { .. }
    ));
}

#[test]
fn prop_all_declared_counts_are_bounded() {
    let encoded = encode_sample_plan();
    let limits = [
        CodecLimits {
            max_tasks: 2,
            ..CodecLimits::unbounded()
        },
        CodecLimits {
            max_dependencies: 1,
            ..CodecLimits::unbounded()
        },
        CodecLimits {
            max_resources: 5,
            ..CodecLimits::unbounded()
        },
        CodecLimits {
            max_key_bytes: 23,
            ..CodecLimits::unbounded()
        },
        CodecLimits {
            max_profile_bytes: 1,
            ..CodecLimits::unbounded()
        },
    ];
    for limit in limits {
        assert!(decode_plan(&encoded, &U64KeyCodec, limit).is_err());
    }
}

#[test]
fn prop_decode_work_never_exceeds_limit() {
    let error = must_err(decode_plan_with_control(
        &encode_sample_plan(),
        &U64KeyCodec,
        CodecLimits::unbounded(),
        CodecControl::with_work_limit(1),
    ));
    assert!(matches!(error, InteropError::WorkLimitExceeded { .. }));
}

#[test]
fn prop_cancellation_has_no_partial_result() {
    let encoded = encode_sample_plan();
    for threshold in [0, 1, encoded.len() as u64 / 2, encoded.len() as u64] {
        let error = must_err(decode_plan_with_control(
            &encoded,
            &U64KeyCodec,
            CodecLimits::unbounded(),
            CodecControl::unlimited().with_cancel_after_work(threshold),
        ));
        assert!(matches!(error, InteropError::Cancelled { .. }));
    }
    let cancelled = AtomicBool::new(true);
    assert!(matches!(
        must_err(decode_plan_with_control(
            &encoded,
            &U64KeyCodec,
            CodecLimits::unbounded(),
            CodecControl::unlimited().with_cancellation(&cancelled),
        )),
        InteropError::Cancelled { .. }
    ));
}

#[test]
fn prop_output_reserves_are_exact() {
    let encoded = must_ok(encode_plan_with_control(
        &plan(8),
        &U64KeyCodec,
        CodecLimits::unbounded(),
        CodecControl::unlimited(),
    ));
    assert_eq!(encoded.metrics().retained_reservations(), 1);
    assert_eq!(
        encoded.value().len() as u64,
        encoded.metrics().published_bytes()
    );
}

#[test]
fn prop_foreign_key_codec_is_rejected() {
    assert!(matches!(
        must_err(decode_plan(
            &encode_sample_plan(),
            &ForeignU64Codec,
            CodecLimits::unbounded()
        )),
        InteropError::ForeignKeyCodec { .. }
    ));
}

#[test]
fn prop_key_decode_reencode_is_exact() {
    for key in [0, 1, u64::MAX / 2, u64::MAX] {
        let source = plan_with_keys(vec![key]);
        let bytes = must_ok(encode_plan(&source, &U64KeyCodec, CodecLimits::unbounded()));
        let decoded = must_ok(decode_plan(&bytes, &U64KeyCodec, CodecLimits::unbounded()));
        assert_eq!(
            bytes,
            must_ok(encode_plan(
                &decoded,
                &U64KeyCodec,
                CodecLimits::unbounded()
            ))
        );
    }
}

#[test]
fn prop_key_codec_collisions_fail_closed() {
    assert!(matches!(
        must_err(encode_plan(
            &plan_with_keys(vec![1, 2]),
            &CollidingCodec,
            CodecLimits::unbounded()
        )),
        InteropError::NonCanonicalKeyCodec
    ));
}

#[test]
fn prop_plan_bytes_bind_every_identity_field() {
    let source = plan(3);
    let baseline = must_ok(encode_plan(&source, &U64KeyCodec, CodecLimits::unbounded()));
    for variant in source.single_field_variants() {
        let changed = must_ok(encode_plan(
            &variant,
            &U64KeyCodec,
            CodecLimits::unbounded(),
        ));
        assert_ne!(baseline, changed);
    }
}

#[test]
fn prop_plan_round_trip_is_exact() {
    for tasks in 0..16 {
        let source = plan(tasks);
        let encoded = must_ok(encode_plan(&source, &U64KeyCodec, CodecLimits::unbounded()));
        assert_eq!(
            source,
            must_ok(decode_plan(
                &encoded,
                &U64KeyCodec,
                CodecLimits::unbounded()
            ))
        );
    }
}

#[test]
fn prop_plan_reencode_is_byte_identical() {
    let encoded = encode_sample_plan();
    let decoded = must_ok(decode_plan(
        &encoded,
        &U64KeyCodec,
        CodecLimits::unbounded(),
    ));
    assert_eq!(
        encoded,
        must_ok(encode_plan(
            &decoded,
            &U64KeyCodec,
            CodecLimits::unbounded()
        ))
    );
}

#[test]
fn prop_dependency_endpoints_are_in_range() {
    let mut encoded = encode_sample_plan();
    let end = encoded.len();
    encoded[end - 8..end - 4].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(matches!(
        must_err(decode_plan(
            &encoded,
            &U64KeyCodec,
            CodecLimits::unbounded()
        )),
        InteropError::NonCanonicalPlan
    ));
}

#[test]
fn prop_dependencies_are_sorted_unique() {
    let source = must_ok(PlanIdentity::new(
        7,
        vec![0_u64, 1, 2],
        vec![(1, 2), (0, 1), (0, 1)],
        vec![(vec![], vec![]); 3],
        vec![1; 3],
        9,
        String::new(),
    ));
    let encoded = must_ok(encode_plan(&source, &U64KeyCodec, CodecLimits::unbounded()));
    assert_eq!(
        source,
        must_ok(decode_plan(
            &encoded,
            &U64KeyCodec,
            CodecLimits::unbounded()
        ))
    );
}

#[test]
fn prop_effect_sets_are_sorted_unique() {
    let source = must_ok(PlanIdentity::new(
        7,
        vec![0_u64],
        vec![],
        vec![(vec![3, 1, 3, 2], vec![9, 8, 9])],
        vec![1],
        9,
        String::new(),
    ));
    let encoded = must_ok(encode_plan(&source, &U64KeyCodec, CodecLimits::unbounded()));
    assert_eq!(
        source,
        must_ok(decode_plan(
            &encoded,
            &U64KeyCodec,
            CodecLimits::unbounded()
        ))
    );
}

#[test]
fn prop_semantic_profile_is_exact_utf8() {
    let source = must_ok(PlanIdentity::new(
        9,
        vec![0_u64],
        vec![],
        vec![(vec![], vec![])],
        vec![1],
        1,
        "λ-naïve-🦀".to_owned(),
    ));
    let encoded = must_ok(encode_plan(&source, &U64KeyCodec, CodecLimits::unbounded()));
    assert_eq!(
        source,
        must_ok(decode_plan(
            &encoded,
            &U64KeyCodec,
            CodecLimits::unbounded()
        ))
    );
}

#[test]
fn prop_digest_domains_are_separate() {
    assert_ne!(DIGEST_PLAN_CONTEXT, DIGEST_CHECKPOINT_CONTEXT);
    let bytes = b"identical input";
    assert_ne!(
        digest_plan(bytes).into_bytes(),
        digest_checkpoint(bytes).into_bytes()
    );
}

#[test]
fn prop_digest_binds_schema_length_and_bytes() {
    let bytes = encode_sample_plan();
    let baseline = digest_plan(&bytes);
    for index in 0..bytes.len() {
        let mut changed = bytes.clone();
        changed[index] ^= 1;
        assert_ne!(baseline, digest_plan(&changed));
    }
    let mut longer = bytes.clone();
    longer.push(0);
    assert_ne!(baseline, digest_plan(&longer));
}

#[test]
fn prop_digest_mismatch_fails_closed() {
    let encoded = encode_sample_plan();
    assert!(matches!(
        must_err(decode_verified_plan(
            &encoded,
            &U64KeyCodec,
            digest_plan(b"foreign"),
            CodecLimits::unbounded()
        )),
        InteropError::DigestMismatch { .. }
    ));
}

#[test]
fn prop_checkpoint_requires_exact_active_plan() {
    assert!(matches!(
        must_err(decode_checkpoint_for(
            &encode_sample_checkpoint(),
            &plan(3).with_budget(7),
            &U64KeyCodec,
            CodecLimits::unbounded()
        )),
        InteropError::ForeignPlan
    ));
}

#[test]
fn prop_plan_digest_never_replaces_exact_equality() {
    let encoded = encode_sample_checkpoint();
    assert!(matches!(
        must_err(decode_verified_checkpoint_for(
            &encoded,
            &plan(3).with_budget(7),
            &U64KeyCodec,
            digest_checkpoint(&encoded),
            CodecLimits::unbounded()
        )),
        InteropError::ForeignPlan
    ));
}

#[test]
fn prop_checkpoint_counts_are_bounded() {
    let mut encoded = encode_sample_checkpoint();
    replace_u64(&mut encoded, 40, u64::MAX);
    assert!(
        decode_checkpoint_for(&encoded, &plan(3), &U64KeyCodec, CodecLimits::unbounded()).is_err()
    );
    let limits = CodecLimits {
        max_events: 1,
        ..CodecLimits::unbounded()
    };
    assert!(
        decode_checkpoint_for(&encode_sample_checkpoint(), &plan(3), &U64KeyCodec, limits).is_err()
    );
}

#[test]
fn prop_checkpoint_successes_form_task_prefix() {
    for prefix in 0..=3 {
        let source = checkpoint(3, prefix);
        let encoded = must_ok(encode_checkpoint(
            &source,
            &U64KeyCodec,
            CodecLimits::unbounded(),
        ));
        let decoded = must_ok(decode_checkpoint_for(
            &encoded,
            &plan(3),
            &U64KeyCodec,
            CodecLimits::unbounded(),
        ));
        assert_eq!(decoded.next_task_cursor(), prefix);
    }
}

#[test]
fn prop_checkpoint_terminal_is_unique_last() {
    let terminal_report = report(3);
    let source = must_ok(Checkpoint::with_published_prefix(
        &terminal_report,
        terminal_report.observation().len(),
    ));
    let mut encoded = must_ok(encode_checkpoint(
        &source,
        &U64KeyCodec,
        CodecLimits::unbounded(),
    ));
    assert!(
        decode_checkpoint_for(&encoded, &plan(3), &U64KeyCodec, CodecLimits::unbounded()).is_ok()
    );
    encoded.push(1);
    assert!(
        decode_checkpoint_for(&encoded, &plan(3), &U64KeyCodec, CodecLimits::unbounded()).is_err()
    );
}

#[test]
fn prop_resume_cursor_is_derived() {
    for prefix in 0..=8 {
        let source = checkpoint(8, prefix);
        let bytes = must_ok(encode_checkpoint(
            &source,
            &U64KeyCodec,
            CodecLimits::unbounded(),
        ));
        let decoded = must_ok(decode_checkpoint_for(
            &bytes,
            &plan(8),
            &U64KeyCodec,
            CodecLimits::unbounded(),
        ));
        assert_eq!(decoded.next_task_cursor(), prefix);
    }
}

#[test]
fn prop_receipts_are_canonical_prefix() {
    let source_report = report(4);
    for published in 0..=source_report.observation().len() {
        let source = must_ok(Checkpoint::with_published_prefix(&source_report, published));
        let encoded = must_ok(encode_checkpoint(
            &source,
            &U64KeyCodec,
            CodecLimits::unbounded(),
        ));
        assert!(
            decode_checkpoint_for(&encoded, &plan(4), &U64KeyCodec, CodecLimits::unbounded())
                .is_ok()
        );
    }
}

#[test]
fn prop_unknown_event_kind_is_rejected() {
    let mut encoded = encode_sample_checkpoint();
    let last = encoded.len() - 1;
    encoded[last] = 255;
    assert!(matches!(
        must_err(decode_checkpoint_for(
            &encoded,
            &plan(3),
            &U64KeyCodec,
            CodecLimits::unbounded()
        )),
        InteropError::UnknownEventKind { .. }
    ));
}

#[test]
fn prop_checkpoint_wire_contains_no_payload() {
    let marker = 0xfeed_face_cafe_beef_u64;
    let source_report = must_ok(ResumeMachine::run(ProtocolInput::new(
        plan(1),
        vec![Outcome::Success(marker)],
    )));
    let source = must_ok(Checkpoint::from_committed_prefix(&source_report, 1));
    let encoded = must_ok(encode_checkpoint(
        &source,
        &U64KeyCodec,
        CodecLimits::unbounded(),
    ));
    assert!(!encoded
        .windows(8)
        .any(|window| window == marker.to_le_bytes()));
}

#[test]
fn prop_checkpoint_round_trip_is_exact() {
    for tasks in 0..8 {
        for prefix in 0..=tasks {
            let source = checkpoint(tasks, prefix);
            let encoded = must_ok(encode_checkpoint(
                &source,
                &U64KeyCodec,
                CodecLimits::unbounded(),
            ));
            assert_eq!(
                source,
                must_ok(decode_checkpoint_for(
                    &encoded,
                    &plan(tasks),
                    &U64KeyCodec,
                    CodecLimits::unbounded()
                ))
            );
        }
    }
}

#[test]
fn prop_decoded_resume_matches_serial() {
    for tasks in 0..8 {
        let uninterrupted = report(tasks);
        let outcomes = (0..tasks as u64).map(Outcome::Success).collect::<Vec<_>>();
        for prefix in 0..=tasks {
            let source = must_ok(Checkpoint::from_committed_prefix(&uninterrupted, prefix));
            let bytes = must_ok(encode_checkpoint(
                &source,
                &U64KeyCodec,
                CodecLimits::unbounded(),
            ));
            let decoded = must_ok(decode_checkpoint_for(
                &bytes,
                &plan(tasks),
                &U64KeyCodec,
                CodecLimits::unbounded(),
            ));
            let resumed = must_ok(ResumeMachine::run(ProtocolInput::resume(
                plan(tasks),
                outcomes.clone(),
                decoded,
            )));
            assert_eq!(resumed.observation(), uninterrupted.observation());
        }
    }
}

#[test]
fn prop_replay_checkpoint_bytes_are_stable() {
    for prefix in 0..=8 {
        let source = checkpoint(8, prefix);
        let bytes = must_ok(encode_checkpoint(
            &source,
            &U64KeyCodec,
            CodecLimits::unbounded(),
        ));
        let decoded = must_ok(decode_checkpoint_for(
            &bytes,
            &plan(8),
            &U64KeyCodec,
            CodecLimits::unbounded(),
        ));
        assert_eq!(
            bytes,
            must_ok(encode_checkpoint(
                &decoded,
                &U64KeyCodec,
                CodecLimits::unbounded()
            ))
        );
    }
}

#[test]
fn small_stack_deep_codec_lifecycle() {
    let handle = must_ok(thread::Builder::new().stack_size(64 * 1024).spawn(|| {
        let source = plan(100_000);
        let bytes = must_ok(encode_plan(&source, &U64KeyCodec, CodecLimits::unbounded()));
        assert_eq!(
            source,
            must_ok(decode_plan(&bytes, &U64KeyCodec, CodecLimits::unbounded()))
        );
    }));
    handle
        .join()
        .unwrap_or_else(|payload| std::panic::resume_unwind(payload));
}

#[test]
fn prop_codec_work_and_heap_are_linear() {
    let mut previous = None;
    for tasks in [1, 2, 4, 8, 16, 32] {
        let encoded = must_ok(encode_plan_with_control(
            &plan(tasks),
            &U64KeyCodec,
            CodecLimits::unbounded(),
            CodecControl::unlimited(),
        ));
        if let Some((old_tasks, old_work, old_heap)) = previous {
            assert!(encoded.metrics().work() <= old_work * tasks as u64 / old_tasks as u64 + 512);
            assert!(encoded.metrics().peak_heap_bytes() <= old_heap * tasks / old_tasks + 512);
        }
        previous = Some((
            tasks,
            encoded.metrics().work(),
            encoded.metrics().peak_heap_bytes(),
        ));
    }
}

#[test]
fn prop_parallel_encodes_are_deterministic() {
    let source = Arc::new(plan(1_024));
    let expected = encode_sample(&source);
    let mut handles = Vec::with_capacity(8);
    for _ in 0..8 {
        let source = Arc::clone(&source);
        handles.push(thread::spawn(move || encode_sample(&source)));
    }
    for handle in handles {
        let observed = handle
            .join()
            .unwrap_or_else(|payload| std::panic::resume_unwind(payload));
        assert_eq!(observed, expected);
    }
}

#[test]
fn prop_malformed_bytes_never_panic_or_publish() {
    let canonical = encode_sample_plan();
    for end in 0..canonical.len() {
        assert!(decode_plan(&canonical[..end], &U64KeyCodec, CodecLimits::unbounded()).is_err());
    }
    for index in 0..canonical.len() {
        let mut changed = canonical.clone();
        changed[index] ^= 0xff;
        let _ = decode_plan(&changed, &U64KeyCodec, CodecLimits::unbounded());
    }
}

fn encode_sample(source: &PlanIdentity<u64, String>) -> Vec<u8> {
    must_ok(encode_plan(source, &U64KeyCodec, CodecLimits::unbounded()))
}

struct ForeignU64Codec;

impl CanonicalKeyCodec<u64> for ForeignU64Codec {
    fn id(&self) -> KeyCodecId {
        KeyCodecId::new([0x55; 32])
    }

    fn encoded_len(&self, _key: &u64) -> Result<usize, KeyCodecError> {
        Ok(8)
    }

    fn encode_into(&self, key: &u64, output: &mut Vec<u8>) -> Result<(), KeyCodecError> {
        output.extend_from_slice(&key.to_be_bytes());
        Ok(())
    }

    fn decode(&self, bytes: &[u8]) -> Result<u64, KeyCodecError> {
        let array: [u8; 8] = bytes.try_into().map_err(|_| KeyCodecError::Rejected)?;
        Ok(u64::from_be_bytes(array))
    }
}

struct CollidingCodec;

impl CanonicalKeyCodec<u64> for CollidingCodec {
    fn id(&self) -> KeyCodecId {
        KeyCodecId::new([0x77; 32])
    }

    fn encoded_len(&self, _key: &u64) -> Result<usize, KeyCodecError> {
        Ok(1)
    }

    fn encode_into(&self, _key: &u64, output: &mut Vec<u8>) -> Result<(), KeyCodecError> {
        output.push(0);
        Ok(())
    }

    fn decode(&self, _bytes: &[u8]) -> Result<u64, KeyCodecError> {
        Ok(0)
    }
}

#[test]
fn control_cancellation_flag_can_be_cleared_after_return() {
    let cancelled = AtomicBool::new(true);
    let encoded = encode_sample_plan();
    assert!(decode_plan_with_control(
        &encoded,
        &U64KeyCodec,
        CodecLimits::unbounded(),
        CodecControl::unlimited().with_cancellation(&cancelled)
    )
    .is_err());
    cancelled.store(false, Ordering::Relaxed);
    assert!(decode_plan(&encoded, &U64KeyCodec, CodecLimits::unbounded()).is_ok());
}

#[test]
fn control_checkpoint_reports_complete_metrics() {
    let encoded = must_ok(encode_checkpoint_with_control(
        &checkpoint(4, 3),
        &U64KeyCodec,
        CodecLimits::unbounded(),
        CodecControl::unlimited(),
    ));
    let decoded = must_ok(decode_checkpoint_with_control(
        encoded.value(),
        &U64KeyCodec,
        CodecLimits::unbounded(),
        CodecControl::unlimited(),
    ));
    assert!(encoded.metrics().published());
    assert!(decoded.metrics().published());
}
