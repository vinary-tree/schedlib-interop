use std::{
    collections::BTreeSet,
    fmt::Debug,
    sync::atomic::{AtomicUsize, Ordering},
};

use proptest::prelude::*;
use schedlib::durable::{Checkpoint, DurableOutcome, PlanIdentity, ProtocolInput, ResumeMachine};
use schedlib_interop::{
    decode_checkpoint_for, decode_plan, encode_checkpoint, encode_plan, encode_plan_with_control,
    CanonicalKeyCodec, CodecControl, CodecLimits, InteropError, KeyCodecError, KeyCodecId,
    U64KeyCodec,
};

fn must_ok<T, E: Debug>(result: Result<T, E>) -> T {
    match result {
        Ok(value) => value,
        Err(error) => panic!("expected success, received {error:?}"),
    }
}

fn generated_plan(keys: BTreeSet<u64>, budget: u64, profile: String) -> PlanIdentity<u64, String> {
    let keys: Vec<_> = keys.into_iter().collect();
    let dependencies = keys.windows(2).map(|pair| (pair[0], pair[1])).collect();
    let effects = keys
        .iter()
        .map(|key| {
            (
                vec![key.rotate_left(7), key.rotate_left(7), *key],
                vec![key.rotate_right(11)],
            )
        })
        .collect();
    let costs = keys.iter().map(|key| key ^ budget).collect();
    must_ok(PlanIdentity::new(
        19,
        keys,
        dependencies,
        effects,
        costs,
        budget,
        profile,
    ))
}

fn indexed_plan(tasks: usize) -> PlanIdentity<u64, String> {
    generated_plan(
        (0..u64::try_from(tasks).unwrap_or(u64::MAX)).collect(),
        31,
        String::from("property-checkpoint"),
    )
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 128,
        max_shrink_iters: 4_096,
        ..ProptestConfig::default()
    })]

    #[test]
    fn arbitrary_canonical_plan_round_trips_exactly(
        keys in prop::collection::btree_set(any::<u64>(), 0..64),
        budget in any::<u64>(),
        profile_chars in prop::collection::vec(any::<char>(), 0..32),
    ) {
        let profile: String = profile_chars.into_iter().collect();
        let source = generated_plan(keys, budget, profile);
        let bytes = must_ok(encode_plan(
            &source,
            &U64KeyCodec,
            CodecLimits::unbounded(),
        ));
        let decoded = must_ok(decode_plan(
            &bytes,
            &U64KeyCodec,
            CodecLimits::unbounded(),
        ));
        prop_assert_eq!(&decoded, &source);
        prop_assert_eq!(
            must_ok(encode_plan(
                &decoded,
                &U64KeyCodec,
                CodecLimits::unbounded(),
            )),
            bytes,
        );
    }

    #[test]
    fn arbitrary_bytes_never_panic_or_publish_partial_plan(
        bytes in prop::collection::vec(any::<u8>(), 0..1_024),
    ) {
        let _result = decode_plan::<u64, _>(
            &bytes,
            &U64KeyCodec,
            CodecLimits::unbounded(),
        );
    }

    #[test]
    fn arbitrary_single_byte_mutation_is_rejected_or_canonical(
        keys in prop::collection::btree_set(any::<u64>(), 1..32),
        choice in any::<usize>(),
        bit in 0_u8..8,
    ) {
        let source = generated_plan(keys, 73, String::from("mutation-property"));
        let mut bytes = must_ok(encode_plan(
            &source,
            &U64KeyCodec,
            CodecLimits::unbounded(),
        ));
        let index = choice % bytes.len();
        bytes[index] ^= 1_u8 << bit;
        if let Ok(decoded) = decode_plan(&bytes, &U64KeyCodec, CodecLimits::unbounded()) {
            prop_assert_eq!(
                must_ok(encode_plan(
                    &decoded,
                    &U64KeyCodec,
                    CodecLimits::unbounded(),
                )),
                bytes,
            );
        }
    }

    #[test]
    fn arbitrary_checkpoint_prefix_refines_uninterrupted_execution(
        tasks in 0_usize..64,
        prefix_seed in any::<usize>(),
    ) {
        let plan = indexed_plan(tasks);
        let outcomes = (0..u64::try_from(tasks).unwrap_or(u64::MAX))
            .map(DurableOutcome::<u64, u64, u64>::Success)
            .collect::<Vec<_>>();
        let report = must_ok(ResumeMachine::run(ProtocolInput::new(
            plan.clone(),
            outcomes,
        )));
        let prefix = prefix_seed % tasks.saturating_add(1);
        let checkpoint = must_ok(Checkpoint::from_committed_prefix(&report, prefix));
        let bytes = must_ok(encode_checkpoint(
            &checkpoint,
            &U64KeyCodec,
            CodecLimits::unbounded(),
        ));
        let decoded = must_ok(decode_checkpoint_for(
            &bytes,
            &plan,
            &U64KeyCodec,
            CodecLimits::unbounded(),
        ));
        prop_assert_eq!(decoded.next_task_cursor(), prefix);
    }
}

#[test]
fn hidden_checkpoint_corruption_is_not_normalized() {
    let corrupt = Checkpoint::corrupt(indexed_plan(3));
    assert!(matches!(
        encode_checkpoint(&corrupt, &U64KeyCodec, CodecLimits::unbounded()),
        Err(InteropError::NonCanonicalPlan)
    ));
}

#[test]
fn row_resource_count_cannot_evade_declared_work() {
    let source = generated_plan([7].into_iter().collect(), 1, String::new());
    let mut bytes = must_ok(encode_plan(&source, &U64KeyCodec, CodecLimits::unbounded()));
    let first_read_count = 112 + 8 + 8 + 8;
    bytes[first_read_count..first_read_count + 4].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(matches!(
        decode_plan(&bytes, &U64KeyCodec, CodecLimits::unbounded()),
        Err(InteropError::NonCanonicalPlan)
    ));
}

#[test]
fn encode_work_limit_precedes_variable_key_measurement() {
    let codec = CountingU64Codec {
        length_calls: AtomicUsize::new(0),
    };
    let result = encode_plan_with_control(
        &indexed_plan(1_000),
        &codec,
        CodecLimits::unbounded(),
        CodecControl::with_work_limit(1),
    );
    assert!(matches!(
        result,
        Err(InteropError::WorkLimitExceeded { .. })
    ));
    assert_eq!(codec.length_calls.load(Ordering::Relaxed), 0);
}

struct CountingU64Codec {
    length_calls: AtomicUsize,
}

impl CanonicalKeyCodec<u64> for CountingU64Codec {
    fn id(&self) -> KeyCodecId {
        CanonicalKeyCodec::<u64>::id(&U64KeyCodec)
    }

    fn encoded_len(&self, key: &u64) -> Result<usize, KeyCodecError> {
        self.length_calls.fetch_add(1, Ordering::Relaxed);
        CanonicalKeyCodec::<u64>::encoded_len(&U64KeyCodec, key)
    }

    fn encode_into(&self, key: &u64, output: &mut Vec<u8>) -> Result<(), KeyCodecError> {
        CanonicalKeyCodec::<u64>::encode_into(&U64KeyCodec, key, output)
    }

    fn decode(&self, bytes: &[u8]) -> Result<u64, KeyCodecError> {
        CanonicalKeyCodec::<u64>::decode(&U64KeyCodec, bytes)
    }
}
