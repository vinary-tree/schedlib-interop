---- MODULE SchedlibInterop ----
EXTENDS Naturals, Sequences, FiniteSets, TLC

CONSTANTS MaxBytes,
          ActualBytes,
          DeclaredBytes,
          WorkLimit,
          HeaderOK,
          CountsOK,
          ArithmeticOK,
          KeyRoundTrip,
          CanonicalPlanOK,
          CheckpointOK,
          DigestCoversAll,
          DigestOK,
          PlanEqual,
          ReplayEqual,
          ConcurrentEqual,
          CancelEnabled,
          CancelAfter

Phases == {"Header", "Scanned", "Allocated", "Decoded", "Verified",
           "Published", "Rejected", "Cancelled"}
TerminalPhases == {"Published", "Rejected", "Cancelled"}

VARIABLES phase, cursor, work, allocated, published
vars == <<phase, cursor, work, allocated, published>>

ByteAdmission == ActualBytes <= MaxBytes
ExactInput == DeclaredBytes = ActualBytes
StructuralAdmission == HeaderOK /\ CountsOK /\ ArithmeticOK /\ ExactInput
CanonicalAdmission == KeyRoundTrip /\ CanonicalPlanOK /\ CheckpointOK
CompleteAdmission == ByteAdmission /\ StructuralAdmission /\ CanonicalAdmission
CancelNow == CancelEnabled /\ cursor >= CancelAfter

Init ==
  /\ phase = "Header"
  /\ cursor = 0
  /\ work = 0
  /\ allocated = FALSE
  /\ published = FALSE

RejectByteLimit ==
  /\ phase = "Header"
  /\ ~ByteAdmission
  /\ phase' = "Rejected"
  /\ UNCHANGED <<cursor, work, allocated, published>>

RejectFixedHeader ==
  /\ phase = "Header"
  /\ ByteAdmission
  /\ ~StructuralAdmission
  /\ phase' = "Rejected"
  /\ UNCHANGED <<cursor, work, allocated, published>>

CancelBeforeScan ==
  /\ phase = "Header"
  /\ ByteAdmission
  /\ StructuralAdmission
  /\ CancelNow
  /\ phase' = "Cancelled"
  /\ UNCHANGED <<cursor, work, allocated, published>>

BeginScan ==
  /\ phase = "Header"
  /\ ByteAdmission
  /\ StructuralAdmission
  /\ ~CancelNow
  /\ phase' = "Scanned"
  /\ UNCHANGED <<cursor, work, allocated, published>>

ScanByte ==
  /\ phase = "Scanned"
  /\ cursor < ActualBytes
  /\ ~CancelNow
  /\ cursor' = cursor + 1
  /\ work' = work + 1
  /\ UNCHANGED <<phase, allocated, published>>

CancelScan ==
  /\ phase = "Scanned"
  /\ CancelNow
  /\ phase' = "Cancelled"
  /\ UNCHANGED <<cursor, work, allocated, published>>

RejectCanonical ==
  /\ phase = "Scanned"
  /\ cursor = ActualBytes
  /\ ~CancelNow
  /\ ~CanonicalAdmission
  /\ phase' = "Rejected"
  /\ UNCHANGED <<cursor, work, allocated, published>>

Allocate ==
  /\ phase = "Scanned"
  /\ cursor = ActualBytes
  /\ ~CancelNow
  /\ CanonicalAdmission
  /\ work <= WorkLimit
  /\ phase' = "Allocated"
  /\ allocated' = TRUE
  /\ UNCHANGED <<cursor, work, published>>

RejectWork ==
  /\ phase = "Scanned"
  /\ cursor = ActualBytes
  /\ ~CancelNow
  /\ CanonicalAdmission
  /\ work > WorkLimit
  /\ phase' = "Rejected"
  /\ UNCHANGED <<cursor, work, allocated, published>>

Decode ==
  /\ phase = "Allocated"
  /\ ~CancelNow
  /\ phase' = "Decoded"
  /\ UNCHANGED <<cursor, work, allocated, published>>

Verify ==
  /\ phase = "Decoded"
  /\ ~CancelNow
  /\ DigestOK
  /\ PlanEqual
  /\ DigestCoversAll
  /\ phase' = "Verified"
  /\ UNCHANGED <<cursor, work, allocated, published>>

RejectDigestOrPlan ==
  /\ phase = "Decoded"
  /\ ~CancelNow
  /\ ~(DigestOK /\ PlanEqual /\ DigestCoversAll)
  /\ phase' = "Rejected"
  /\ UNCHANGED <<cursor, work, allocated, published>>

CancelLate ==
  /\ phase \in {"Allocated", "Decoded", "Verified"}
  /\ CancelNow
  /\ phase' = "Cancelled"
  /\ UNCHANGED <<cursor, work, allocated, published>>

Publish ==
  /\ phase = "Verified"
  /\ ~CancelNow
  /\ ReplayEqual
  /\ ConcurrentEqual
  /\ phase' = "Published"
  /\ published' = TRUE
  /\ UNCHANGED <<cursor, work, allocated>>

RejectRefinement ==
  /\ phase = "Verified"
  /\ ~CancelNow
  /\ ~(ReplayEqual /\ ConcurrentEqual)
  /\ phase' = "Rejected"
  /\ UNCHANGED <<cursor, work, allocated, published>>

RemainTerminal ==
  /\ phase \in TerminalPhases
  /\ UNCHANGED vars

Next ==
  \/ RejectByteLimit
  \/ RejectFixedHeader
  \/ CancelBeforeScan
  \/ BeginScan
  \/ ScanByte
  \/ CancelScan
  \/ RejectCanonical
  \/ Allocate
  \/ RejectWork
  \/ Decode
  \/ Verify
  \/ RejectDigestOrPlan
  \/ CancelLate
  \/ Publish
  \/ RejectRefinement
  \/ RemainTerminal

Spec == Init /\ [][Next]_vars /\ WF_vars(Next)

TypeOK ==
  /\ phase \in Phases
  /\ cursor \in Nat
  /\ cursor <= ActualBytes
  /\ work \in Nat
  /\ allocated \in BOOLEAN
  /\ published \in BOOLEAN

HeaderMustMatch == published => HeaderOK
ExactLength == published => ExactInput
ArithmeticIsChecked == published => ArithmeticOK
AdmissionPrecedesAllocation == allocated => CompleteAdmission
WorkNeverExceedsBudget == allocated => work <= WorkLimit
CancelledHasNoPublication == (phase = "Cancelled") => ~published
CanonicalRoundTrip == published => CanonicalAdmission
ExactPlanIdentity == published => PlanEqual
CanonicalPlan == published => CanonicalPlanOK
DigestDomainSeparation == "schedlib-plan-v1" # "schedlib-checkpoint-v1"
DigestCoversFrame == published => DigestCoversAll
DigestMismatchRejects == ~DigestOK => ~published
CanonicalCheckpoint == published => CheckpointOK
ReplayMatchesSerial == published => ReplayEqual
ConcurrentCallsAreDeterministic == published => ConcurrentEqual
MalformedFailsClosed == ~CompleteAdmission => ~published
NoPartialPublication == published => phase = "Published"
EventuallyTerminal == <>(phase \in TerminalPhases)

====
