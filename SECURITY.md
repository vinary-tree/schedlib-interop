# Security policy

Report suspected vulnerabilities privately to `dylon@f1r3fly.io`. Include the
affected format version, smallest reproducing frame, selected `KeyCodecId`,
limits, expected result, and observed result. Do not attach production secrets
or unredacted persisted artifacts.

The supported line is 0.1.x. Malformed input must return a typed error without
panic or partial publication. Digest collision resistance is not treated as
structural equality or authentication. See the
[threat model](docs/security/threat-model.md) for the complete boundary.
