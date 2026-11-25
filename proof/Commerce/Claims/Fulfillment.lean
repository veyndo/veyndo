-- Fulfillment contracts for the extracted production policies.
import Commerce.Generated
namespace CommerceKernel

/-- Terminal states prohibit operational edits. -/
theorem terminal_edit_denied : order_edit_admissible true = false := by rfl

/-- Completion entails acceptable payment and complete physical deliveries. -/
theorem completion_safe (terminal payment deliveries : Bool)
    (h : completion_admissible terminal payment deliveries = true) :
    terminal = false ∧ payment = true ∧ deliveries = true := by
  cases terminal <;> cases payment <;> cases deliveries <;> simp_all [completion_admissible]

/-- Cancellation respects provider, refund and return constraints. -/
theorem cancellation_safe (terminal external refund deliveriesOpen : Bool)
    (h : cancellation_admissible terminal external refund deliveriesOpen = true) :
    terminal = false ∧ external = false ∧ refund = false ∧ deliveriesOpen = true := by
  cases terminal <;> cases external <;> cases refund <;> cases deliveriesOpen <;> simp_all [cancellation_admissible]

/-- Manual commands cannot confirm external provider payments. -/
theorem manual_payment_safe (terminal external pending targetPaid : Bool)
    (h : manual_payment_admissible terminal external pending targetPaid = true) :
    terminal = false ∧ external = false ∧ pending = true ∧ targetPaid = true := by
  cases terminal <;> cases external <;> cases pending <;> cases targetPaid <;> simp_all [manual_payment_admissible]

/-- Blocked orders never grant downloads. -/
theorem download_blocked_denied (paid simulated : Bool) :
    download_admissible true paid simulated = false := by simp [download_admissible]

/-- Downloads require confirmed payment or explicit simulated authorization. -/
theorem download_payment_required (blocked paid simulated : Bool)
    (h : download_admissible blocked paid simulated = true) :
    blocked = false ∧ (paid = true ∨ simulated = true) := by
  cases blocked <;> cases paid <;> cases simulated <;> simp_all [download_admissible]

/-- Receipts must match the amount and currency and confirm the outcome. -/
theorem receipt_exact (expected received : Nat) (currency confirmed : Bool)
    (h : receipt_admissible expected received currency confirmed = true) :
    expected = received ∧ currency = true ∧ confirmed = true := by
  simpa [receipt_admissible, and_assoc] using h

/-- Nonterminal operational edits remain possible. -/
theorem order_edit_exact (terminal : Bool) : order_edit_admissible terminal = true ↔ terminal = false := by
  cases terminal <;> simp [order_edit_admissible]

/-- Ready nonterminal orders can complete. -/
theorem completion_exact (terminal payment deliveries : Bool) :
    completion_admissible terminal payment deliveries = true ↔
    terminal = false ∧ payment = true ∧ deliveries = true := by
  cases terminal <;> cases payment <;> cases deliveries <;> simp [completion_admissible]

/-- Native cancellation is admitted precisely within the business guard. -/
theorem cancellation_exact (terminal external refund deliveries : Bool) :
    cancellation_admissible terminal external refund deliveries = true ↔
    terminal = false ∧ external = false ∧ refund = false ∧ deliveries = true := by
  cases terminal <;> cases external <;> cases refund <;> cases deliveries <;> simp [cancellation_admissible]

/-- Valid native manual payments remain possible. -/
theorem manual_payment_exact (terminal external pending targetPaid : Bool) :
    manual_payment_admissible terminal external pending targetPaid = true ↔
    terminal = false ∧ external = false ∧ pending = true ∧ targetPaid = true := by
  cases terminal <;> cases external <;> cases pending <;> cases targetPaid <;> simp [manual_payment_admissible]

/-- Unblocked paid/simulated orders receive their permitted download. -/
theorem download_exact (blocked paid simulated : Bool) :
    download_admissible blocked paid simulated = true ↔
    blocked = false ∧ (paid = true ∨ simulated = true) := by
  cases blocked <;> cases paid <;> cases simulated <;> simp [download_admissible]

/-- Exact matching receipts are accepted as well as mismatches rejected. -/
theorem receipt_exact_behavior (expected received : Nat) (currency confirmed : Bool) :
    receipt_admissible expected received currency confirmed = true ↔
    expected = received ∧ currency = true ∧ confirmed = true := by
  simp [receipt_admissible, and_assoc]

/-- Release accepts exactly uncaptured attempts or an explicitly voided authorization. -/
theorem reservation_release_exact (uncaptured authorized voidConfirmed : Bool) :
    reservation_release_admissible uncaptured authorized voidConfirmed = (uncaptured || authorized && voidConfirmed) := by rfl

/-- Uncertain authorizations cannot restore inventory. -/
theorem reservation_authorized_unvoided : reservation_release_admissible false true false = false := by rfl

/-- A later capture cannot be hidden by a void flag. -/
theorem reservation_captured_denied (voidConfirmed : Bool) : reservation_release_admissible false false voidConfirmed = false := by
  cases voidConfirmed <;> simp [reservation_release_admissible]

/-- Full ledger transition relation, including confirmed late capture and refund progression. -/
theorem payment_transition_exact (current next : Nat) :
    payment_transition_admissible current next = true ↔
    current ≤ 10 ∧ next ≤ 10 ∧ (current = next ∨
    (current ≤ 3 ∧ next > current ∧ next ≤ 4) ∨
    (current ≤ 3 ∧ (next = 7 ∨ next = 8)) ∨
    ((current = 4 ∨ current = 5 ∨ current = 9) ∧ (next = 5 ∨ next = 6)) ∨
    ((current = 7 ∨ current = 8) ∧ next = 9) ∨
    (current ≤ 3 ∧ next = 10) ∨
    (current = 10 ∧ (next = 4 ∨ next = 5 ∨ next = 6))) := by
  simp [payment_transition_admissible, and_assoc, or_assoc]

theorem payment_refunded_terminal (next : Nat) :
    payment_transition_admissible 6 next = true ↔ next = 6 := by
  simp [payment_transition_admissible]; omega

theorem payment_pending_capture : payment_transition_admissible 0 4 = true := by rfl

/-- Exact package-subset admission; async allocation and carrier effects remain tested boundaries. -/
theorem delivery_split_exact (is_open terminal : Bool) (available moving : Nat) :
    delivery_split_admissible is_open terminal available moving = true ↔
    is_open = true ∧ terminal = false ∧ 0 < moving ∧ moving < available := by
  cases is_open <;> cases terminal <;> simp [delivery_split_admissible, and_assoc]

theorem delivery_split_closed_denied (terminal : Bool) (available moving : Nat) :
    delivery_split_admissible false terminal available moving = false := by
  simp [delivery_split_admissible]

theorem delivery_split_all_denied (is_open terminal : Bool) (available : Nat) :
    delivery_split_admissible is_open terminal available available = false := by
  simp [delivery_split_admissible]

/-- Native package intent admission, not proof of a carrier protocol or external charge. -/
theorem carrier_booking_exact (is_open closed labelled occupied same : Bool) :
    carrier_booking_admissible is_open closed labelled occupied same = true ↔
    is_open = true ∧ closed = false ∧ labelled = false ∧ (occupied = false ∨ same = true) := by
  cases is_open <;> cases closed <;> cases labelled <;> cases occupied <;> cases same <;> simp [carrier_booking_admissible]

theorem carrier_booking_occupied_denied (is_open closed labelled : Bool) :
    carrier_booking_admissible is_open closed labelled true false = false := by
  simp [carrier_booking_admissible]


/-! ### Order state machine over transition sequences -/

/-- Exact order transition admission: a listed edge into a listed state from a nonterminal state. -/
theorem order_transition_exact (edge listed terminal : Bool) :
    order_transition_admissible edge listed terminal = true ↔
    edge = true ∧ listed = true ∧ terminal = false := by
  cases edge <;> cases listed <;> cases terminal <;> simp [order_transition_admissible]

theorem order_transition_terminal_denied (edge listed : Bool) :
    order_transition_admissible edge listed true = false := by
  cases edge <;> cases listed <;> rfl

/-- A configurable order machine as production reads it: listed states, terminal states
(machine-declared plus the system review states) and listed edges, with states as codes. -/
structure OrderMachineModel where
  listed : Nat → Bool
  terminal : Nat → Bool
  edge : Nat → Nat → Bool

/-- A merchant request: move the order to a state, or change its payment or a delivery. -/
inductive OrderRequest where
  | move (target : Nat)
  | fulfil

/-- Order state code and the number of applied fulfilment changes; refused requests change nothing. -/
def orderStep (m : OrderMachineModel) (s : Nat × Nat) : OrderRequest → Nat × Nat
  | .move t =>
    if order_transition_admissible (m.edge s.1 t) (m.listed t) (m.terminal s.1) then (t, s.2) else s
  | .fulfil => if order_edit_admissible (m.terminal s.1) then (s.1, s.2 + 1) else s

def orderRun (m : OrderMachineModel) (s : Nat × Nat) (requests : List OrderRequest) : Nat × Nat :=
  requests.foldl (orderStep m) s

theorem order_step_listed (m : OrderMachineModel) (s : Nat × Nat) (r : OrderRequest)
    (h : m.listed s.1 = true) : m.listed (orderStep m s r).1 = true := by
  cases r with
  | move t =>
    simp only [orderStep]
    split
    · rename_i ht
      exact ((order_transition_exact _ _ _).mp ht).2.1
    · exact h
  | fulfil =>
    simp only [orderStep]
    split <;> exact h

/-- Every state reachable from a listed start by any request sequence is a listed state. -/
theorem order_run_listed (m : OrderMachineModel) (requests : List OrderRequest) :
    ∀ s : Nat × Nat, m.listed s.1 = true → m.listed (orderRun m s requests).1 = true := by
  induction requests with
  | nil => intro s h; exact h
  | cons r rest ih =>
    intro s h
    exact ih (orderStep m s r) (order_step_listed m s r h)

theorem order_step_terminal (m : OrderMachineModel) (s : Nat × Nat) (r : OrderRequest)
    (h : m.terminal s.1 = true) : orderStep m s r = s := by
  cases r with
  | move t => simp [orderStep, h, order_transition_terminal_denied]
  | fulfil => simp [orderStep, h, order_edit_admissible]

/-- A completed or cancelled (terminal) order never changes again: no move and no fulfilment. -/
theorem order_run_terminal_absorbing (m : OrderMachineModel) (requests : List OrderRequest) :
    ∀ s : Nat × Nat, m.terminal s.1 = true → orderRun m s requests = s := by
  induction requests with
  | nil => intro s _; rfl
  | cons r rest ih =>
    intro s h
    have hs := order_step_terminal m s r h
    show orderRun m (orderStep m s r) rest = s
    rw [hs]
    exact ih s h

/-- Once any prefix reaches a terminal state, every later request is without effect. -/
theorem order_run_terminal_final (m : OrderMachineModel) (s : Nat × Nat) (done later : List OrderRequest)
    (h : m.terminal (orderRun m s done).1 = true) :
    orderRun m s (done ++ later) = orderRun m s done := by
  have := order_run_terminal_absorbing m later (orderRun m s done) h
  simpa [orderRun, List.foldl_append] using this

/-- In particular, no payment or delivery change is applied to a terminal order by any sequence. -/
theorem order_run_fulfilment_denied (m : OrderMachineModel) (s : Nat × Nat) (requests : List OrderRequest)
    (h : m.terminal s.1 = true) : (orderRun m s requests).2 = s.2 := by
  rw [order_run_terminal_absorbing m requests s h]

/-! ### Payment ledger over transition sequences -/

/-- Ledger transitions applied in order; refused transitions leave the state unchanged. -/
def ledgerRun (state : Nat) (targets : List Nat) : Nat :=
  targets.foldl (fun s n => if payment_transition_admissible s n then n else s) state

/-- Every ledger state reachable from a listed state is a listed state code. -/
theorem payment_run_listed (targets : List Nat) :
    ∀ s, s ≤ 10 → ledgerRun s targets ≤ 10 := by
  induction targets with
  | nil => intro s h; simpa [ledgerRun] using h
  | cons n rest ih =>
    intro s h
    apply ih
    dsimp only
    split
    · rename_i hn
      exact ((payment_transition_exact s n).mp hn).2.1
    · exact h

/-- Captured, partially refunded, refunded, late-captured or partially captured money never
returns to pending, authorized, cancelled or expired, over any sequence. -/
theorem payment_run_captured_kept (targets : List Nat) :
    ∀ s, (s = 4 ∨ s = 5 ∨ s = 6 ∨ s = 9 ∨ s = 10) →
    (ledgerRun s targets = 4 ∨ ledgerRun s targets = 5 ∨ ledgerRun s targets = 6 ∨
     ledgerRun s targets = 9 ∨ ledgerRun s targets = 10) := by
  induction targets with
  | nil => intro s h; simpa [ledgerRun] using h
  | cons n rest ih =>
    intro s h
    apply ih
    dsimp only
    split
    · rename_i hn
      have := (payment_transition_exact s n).mp hn
      omega
    · exact h

/-- A fully refunded payment is final over any sequence. -/
theorem payment_run_refunded_final (targets : List Nat) : ledgerRun 6 targets = 6 := by
  induction targets with
  | nil => rfl
  | cons n rest ih =>
    show ledgerRun (if payment_transition_admissible 6 n then n else 6) rest = 6
    split
    · rename_i hn
      have := (payment_refunded_terminal n).mp hn
      subst this
      exact ih
    · exact ih

end CommerceKernel
