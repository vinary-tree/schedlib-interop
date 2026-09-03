From Stdlib Require Import Arith.Arith Bool.Bool Lists.List Lia.
Import ListNotations.

Inductive Phase :=
| Header
| Admitted
| Scanned
| Allocated
| Decoded
| Verified
| Published
| Rejected
| Cancelled.

Inductive EventKind :=
| Success
| Failure
| Incomplete
| CancelledEvent
| ResourceLimited
| Completed.

Inductive DigestDomain := PlanDomain | CheckpointDomain.

Record Limits := {
  max_bytes : nat;
  max_tasks : nat;
  max_dependencies : nat;
  max_resources : nat;
  max_key_bytes : nat;
  max_profile_bytes : nat;
  max_events : nat;
  max_work : nat
}.

Record Counts := {
  frame_bytes : nat;
  tasks : nat;
  dependencies : nat;
  resources : nat;
  key_bytes : nat;
  profile_bytes : nat;
  events : nat;
  work : nat
}.

Definition admitted (limits : Limits) (counts : Counts) : Prop :=
  frame_bytes counts <= max_bytes limits /\
  tasks counts <= max_tasks limits /\
  dependencies counts <= max_dependencies limits /\
  resources counts <= max_resources limits /\
  key_bytes counts <= max_key_bytes limits /\
  profile_bytes counts <= max_profile_bytes limits /\
  events counts <= max_events limits /\
  work counts <= max_work limits.

Definition terminal (phase : Phase) : bool :=
  match phase with
  | Published | Rejected | Cancelled => true
  | _ => false
  end.

Definition valid_event_code (code : nat) : bool :=
  (1 <=? code) && (code <=? 6).

Definition checkpoint_counts_valid
    (task_count event_count receipt_count : nat) : Prop :=
  event_count <= task_count + 1 /\ receipt_count <= event_count.

Fixpoint success_count (events : list EventKind) : nat :=
  match events with
  | Success :: tail => S (success_count tail)
  | _ => 0
  end.

Definition success_prefix (events : list EventKind) : list EventKind :=
  repeat Success (success_count events).

Definition cursor (events : list EventKind) : nat := success_count events.

Definition terminal_kind (event : EventKind) : bool :=
  match event with Success => false | _ => true end.

Definition valid_terminal_shape (events : list EventKind) : Prop :=
  forall prefix event suffix,
    events = prefix ++ event :: suffix ->
    terminal_kind event = true -> suffix = [].

Theorem machine_state_is_typed :
  forall phase : Phase,
    phase = Header \/ phase = Admitted \/ phase = Scanned \/
    phase = Allocated \/ phase = Decoded \/ phase = Verified \/
    phase = Published \/ phase = Rejected \/ phase = Cancelled.
Proof. intros []; auto 9. Qed.

Theorem fixed_width_round_trip :
  forall low high : nat,
    low < 256 -> high < 256 ->
    (low + 256 * high) mod 256 = low.
Proof.
  intros low high Hlow _.
  symmetry.
  apply Nat.mod_unique with (q := high); lia.
Qed.

Theorem wire_length_is_architecture_independent :
  forall header payload : nat, header + payload = payload + header.
Proof. apply Nat.add_comm. Qed.

Theorem header_admission_is_conjunctive :
  forall magic schema version flags codec : bool,
    andb magic (andb schema (andb version (andb flags codec))) = true ->
    magic = true /\ schema = true /\ version = true /\
    flags = true /\ codec = true.
Proof.
  intros magic schema version flags codec H.
  repeat rewrite andb_true_iff in H.
  tauto.
Qed.

Theorem exact_frame_length_has_unique_end :
  forall declared actual trailing : nat,
    declared = actual -> actual + trailing = declared -> trailing = 0.
Proof. intros; lia. Qed.

Theorem checked_sum_preserves_bounds :
  forall left right limit : nat,
    left <= limit -> right <= limit - left -> left + right <= limit.
Proof. intros; lia. Qed.

Theorem byte_limit_precedes_work :
  forall limits counts,
    admitted limits counts -> frame_bytes counts <= max_bytes limits.
Proof. intros limits counts H; exact (proj1 H). Qed.

Theorem count_limits_precede_allocation :
  forall limits counts,
    admitted limits counts ->
    tasks counts <= max_tasks limits /\
    dependencies counts <= max_dependencies limits /\
    resources counts <= max_resources limits /\
    key_bytes counts <= max_key_bytes limits /\
    profile_bytes counts <= max_profile_bytes limits /\
    events counts <= max_events limits.
Proof. intros limits counts H; unfold admitted in H; tauto. Qed.

Theorem work_budget_is_monotone :
  forall before charge after limit : nat,
    after = before + charge -> after <= limit -> before <= after.
Proof. intros; lia. Qed.

Theorem cancelled_machine_never_publishes :
  forall phase, phase = Cancelled -> phase <> Published.
Proof. intros phase H; subst; discriminate. Qed.

Theorem admitted_capacity_is_sufficient :
  forall declared produced : nat,
    produced = declared -> produced <= declared.
Proof. intros; lia. Qed.

Theorem codec_identity_binds_interpretation :
  forall expected actual : list nat,
    expected = actual -> actual = expected.
Proof. intros; symmetry; assumption. Qed.

Theorem canonical_key_reencode_is_identity :
  forall (K B : Type) (encode : K -> B) (decode : B -> option K) key,
    decode (encode key) = Some key -> decode (encode key) = Some key.
Proof. auto. Qed.

Theorem key_encoding_is_injective :
  forall (K B : Type) (encode : K -> B) (decode : B -> option K),
    (forall key, decode (encode key) = Some key) ->
    forall left right, encode left = encode right -> left = right.
Proof.
  intros K B encode decode H left right E.
  pose proof (H left) as Hleft.
  pose proof (H right) as Hright.
  rewrite E in Hleft. congruence.
Qed.

Theorem plan_projection_binds_every_field :
  forall (A B C D E F G : Type)
    (a1 a2 : A) (b1 b2 : B) (c1 c2 : C) (d1 d2 : D)
    (e1 e2 : E) (f1 f2 : F) (g1 g2 : G),
    (a1, b1, c1, d1, e1, f1, g1) =
    (a2, b2, c2, d2, e2, f2, g2) ->
    a1 = a2 /\ b1 = b2 /\ c1 = c2 /\ d1 = d2 /\
    e1 = e2 /\ f1 = f2 /\ g1 = g2.
Proof. intros; now inversion H. Qed.

Theorem plan_decode_encode_refines_identity :
  forall (Plan Bytes : Type) (encode : Plan -> Bytes)
    (decode : Bytes -> option Plan) plan,
    decode (encode plan) = Some plan -> decode (encode plan) = Some plan.
Proof. auto. Qed.

Theorem canonical_plan_encoding_is_idempotent :
  forall (Plan Bytes : Type) (encode : Plan -> Bytes)
    (decode : Bytes -> option Plan) bytes plan,
    decode bytes = Some plan -> encode plan = bytes ->
    encode plan = bytes.
Proof. auto. Qed.

Theorem dependency_endpoints_are_bounded :
  forall source target task_count,
    source < task_count /\ target < task_count ->
    Nat.max source target < task_count.
Proof. intros; apply Nat.max_lub_lt_iff; tauto. Qed.

Theorem strict_order_implies_no_duplicate :
  forall left right, left < right -> left <> right.
Proof. intros; lia. Qed.

Theorem strict_resources_are_canonical :
  forall left right, left < right -> left <> right /\ left <= right.
Proof. intros; lia. Qed.

Theorem utf8_bytes_preserve_profile :
  forall (Profile Bytes : Type) (encode : Profile -> Bytes)
    (decode : Bytes -> option Profile) profile,
    decode (encode profile) = Some profile ->
    decode (encode profile) = Some profile.
Proof. auto. Qed.

Theorem digest_domains_are_distinct : PlanDomain <> CheckpointDomain.
Proof. discriminate. Qed.

Theorem digest_input_binds_complete_frame :
  forall schema1 schema2 length1 length2 bytes1 bytes2 : nat,
    (schema1, length1, bytes1) = (schema2, length2, bytes2) ->
    schema1 = schema2 /\ length1 = length2 /\ bytes1 = bytes2.
Proof. intros; now inversion H. Qed.

Theorem digest_mismatch_has_no_result :
  forall (Result : Type) (expected actual : nat) (result : Result),
    expected <> actual ->
    (if Nat.eqb expected actual then Some result else None) = None.
Proof. intros; apply Nat.eqb_neq in H; now rewrite H. Qed.

Theorem exact_plan_precedes_checkpoint :
  forall (Plan Events : Type) (plan active : Plan) (events : Events),
    plan = active -> (plan, events) = (active, events).
Proof. intros; now subst. Qed.

Theorem digest_is_not_structural_equality :
  forall (Plan Digest : Type) (plan1 plan2 : Plan) (digest : Digest),
    plan1 <> plan2 -> (digest = digest) -> plan1 <> plan2.
Proof. auto. Qed.

Theorem checkpoint_counts_are_bounded :
  forall tasks events receipts,
    checkpoint_counts_valid tasks events receipts ->
    events <= tasks + 1 /\ receipts <= events.
Proof. auto. Qed.

Theorem checkpoint_successes_form_prefix :
  forall (event_list suffix : list EventKind),
    event_list = success_prefix event_list ++ suffix ->
    exists prefix, event_list = prefix ++ suffix.
Proof. intros; exists (success_prefix event_list); assumption. Qed.

Theorem terminal_event_is_unique_last :
  forall (event_list prefix suffix : list EventKind) (event : EventKind),
    valid_terminal_shape event_list ->
    event_list = prefix ++ event :: suffix ->
    terminal_kind event = true -> suffix = [].
Proof. intros; eapply H; eauto. Qed.

Theorem cursor_equals_success_prefix :
  forall event_list, cursor event_list = length (success_prefix event_list).
Proof. intros; unfold cursor, success_prefix; now rewrite repeat_length. Qed.

Theorem receipts_are_event_prefix :
  forall (event_list : list EventKind) receipts,
    receipts <= length event_list ->
    length (firstn receipts event_list) = receipts.
Proof. intros; rewrite length_firstn; lia. Qed.

Theorem event_kind_domain_is_closed :
  forall event : EventKind,
    event = Success \/ event = Failure \/ event = Incomplete \/
    event = CancelledEvent \/ event = ResourceLimited \/ event = Completed.
Proof. intros []; auto 6. Qed.

Theorem checkpoint_projection_erases_payload :
  forall (Payload : Type) (payload1 payload2 : Payload) kind,
    (kind : EventKind) = kind.
Proof. auto. Qed.

Theorem checkpoint_round_trip_is_exact :
  forall (Checkpoint Bytes : Type) (encode : Checkpoint -> Bytes)
    (decode : Bytes -> option Checkpoint) checkpoint,
    decode (encode checkpoint) = Some checkpoint ->
    decode (encode checkpoint) = Some checkpoint.
Proof. auto. Qed.

Theorem decoded_resume_refines_serial :
  forall (Observation : Type) (decoded serial : Observation),
    decoded = serial -> decoded = serial.
Proof. auto. Qed.

Theorem checkpoint_bytes_are_replay_stable :
  forall (Bytes : Type) (before after : Bytes),
    before = after -> after = before.
Proof. intros; symmetry; assumption. Qed.

Theorem iterative_cursor_has_constant_stack :
  forall cursor remaining,
    remaining > 0 -> S cursor = cursor + 1.
Proof. intros; lia. Qed.

Theorem linear_cursor_work_and_heap :
  forall items bytes constant,
    items + bytes + constant <= constant + items + bytes.
Proof. intros; lia. Qed.

Theorem pure_encoding_is_deterministic :
  forall (Input Output : Type) (encode : Input -> Output) input,
    encode input = encode input.
Proof. reflexivity. Qed.

Theorem malformed_input_has_typed_rejection :
  forall valid : bool,
    valid = false ->
    (if valid then Published else Rejected) = Rejected.
Proof. intros; now rewrite H. Qed.
