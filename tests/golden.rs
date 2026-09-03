use schedlib::durable::PlanIdentity;
use schedlib_interop::{digest_plan, encode_plan, CodecLimits, U64KeyCodec};

#[test]
fn empty_u64_plan_v1_has_stable_frame_digest() {
    let plan = match PlanIdentity::new(
        7,
        Vec::<u64>::new(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
        9,
        String::new(),
    ) {
        Ok(plan) => plan,
        Err(error) => panic!("valid golden plan rejected: {error}"),
    };
    let bytes = match encode_plan(&plan, &U64KeyCodec, CodecLimits::unbounded()) {
        Ok(bytes) => bytes,
        Err(error) => panic!("valid golden plan encoding rejected: {error}"),
    };
    assert_eq!(bytes.len(), 120);
    assert_eq!(
        digest_plan(&bytes).into_bytes(),
        [
            231, 238, 237, 204, 73, 59, 81, 30, 203, 60, 3, 192, 63, 134, 40, 201, 125, 110, 56,
            89, 160, 210, 90, 42, 205, 128, 254, 21, 127, 11, 249, 52,
        ]
    );
}
