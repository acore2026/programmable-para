(module
  (import "host" "metadata_matches_ue"
    (func $metadata_matches_ue (param i32 i32) (result i32)))
  (import "host" "metadata_is"
    (func $metadata_is (param i32 i32) (result i32)))
  (import "host" "mismatch_action"
    (func $mismatch_action (result i32)))

  ;; Decision codes are owned by the host:
  ;; 0 = ALLOW, 1 = LIMIT_ACCESS, 2 = REJECT
  ;;
  ;; Rel-21 only knows that an AI agent identity and high trust level are required.
  ;; It does not know or care whether later metadata keys exist.
  (func (export "verify") (result i32)
    (if (result i32)
      (i32.and
        (call $metadata_matches_ue (i32.const 1) (i32.const 1))
        (call $metadata_is (i32.const 2) (i32.const 1)))
      (then (i32.const 0))
      (else (call $mismatch_action)))))
