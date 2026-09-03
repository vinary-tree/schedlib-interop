# Changelog

All notable changes follow this file. The project uses semantic versioning.

## 0.1.0

- Add bounded canonical version-one plan and checkpoint codecs.
- Add exact key-codec identities and a built-in big-endian `u64` codec.
- Add domain-separated BLAKE3 plan and checkpoint digests.
- Add iterative cancellation, work accounting, and publication metrics.
- Add forty formally traced refinement properties, shrinking property tests,
  adversarial malformed-input tests, and a 64 KiB stack-safety gate.
