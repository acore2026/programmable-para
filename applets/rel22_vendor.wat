(module
  (import "host" "metadata_matches_ue"
    (func $metadata_matches_ue (param i32 i32) (result i32)))
  (import "host" "metadata_is"
    (func $metadata_is (param i32 i32) (result i32)))
  (import "host" "mismatch_action"
    (func $mismatch_action (result i32)))

  ;; Rel-22 adds vendor verification. The intermediate NF remains unchanged
  ;; because vendor is carried as opaque metadata.
  (func (export "verify") (result i32)
    (if (result i32)
      (i32.and
        (i32.and
          (call $metadata_matches_ue (i32.const 1) (i32.const 1))
          (call $metadata_is (i32.const 2) (i32.const 1)))
        (call $metadata_matches_ue (i32.const 3) (i32.const 3)))
      (then (i32.const 0))
      (else (call $mismatch_action)))))
