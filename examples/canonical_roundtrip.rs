use schedlib::durable::PlanIdentity;
use schedlib_interop::{decode_plan, encode_plan, CodecLimits, U64KeyCodec};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let plan = PlanIdentity::new(
        1,
        vec![10_u64, 20],
        vec![(10, 20)],
        vec![(vec![1], vec![2]), (vec![3], vec![4])],
        vec![1, 1],
        2,
        String::from("example-v1"),
    )?;
    let limits = CodecLimits {
        max_bytes: 16 * 1024,
        max_tasks: 16,
        max_dependencies: 32,
        max_resources: 64,
        max_key_bytes: 256,
        max_profile_bytes: 256,
        max_events: 17,
    };
    let bytes = encode_plan(&plan, &U64KeyCodec, limits)?;
    let decoded = decode_plan(&bytes, &U64KeyCodec, limits)?;
    assert_eq!(decoded, plan);
    Ok(())
}
