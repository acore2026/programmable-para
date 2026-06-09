(module
  ;; Import host functions that query the AMF context:
  ;; metadata_matches_ue(metadata_key, claim_key) -> 1 (true) or 0 (false)
  (import "host" "metadata_matches_ue"
    (func $metadata_matches_ue (param i32 i32) (result i32)))

  ;; metadata_is(metadata_key, expected_value_id) -> 1 (true) or 0 (false)
  (import "host" "metadata_is"
    (func $metadata_is (param i32 i32) (result i32)))

  ;; mismatch_action() -> returns default decision code on mismatch (LIMIT_ACCESS = 1 / REJECT = 2)
  (import "host" "mismatch_action"
    (func $mismatch_action (result i32)))

  ;; Constants mapping defined by the Host (src/wasm.rs):
  ;; - Keys:
  ;;   1 = aiAgentId
  ;;   2 = trustLevel
  ;;   3 = vendor
  ;; - Predefined string values:
  ;;   1 = "high"

  ;; The verify() entry point called by the AMF host.
  ;; Returns: 0 (ALLOW), 1 (LIMIT_ACCESS), or 2 (REJECT)
  (func (export "verify") (result i32)
    (if (result i32)
      (i32.and
        (i32.and
          ;; 1. Check if Subscription's aiAgentId (1) matches UE's aiAgentId claim (1)
          (call $metadata_matches_ue (i32.const 1) (i32.const 1))
          ;; 2. Check if Subscription's trustLevel (2) is "high" (1)
          (call $metadata_is (i32.const 2) (i32.const 1)))
        ;; 3. Check if Subscription's vendor (3) matches UE's vendor claim (3)
        (call $metadata_matches_ue (i32.const 3) (i32.const 3)))
      (then
        ;; If all conditions match, return 0 (ALLOW)
        (i32.const 0))
      (else
        ;; If there is a mismatch, execute the host's configured mismatch fallback policy
        (call $mismatch_action)))))
