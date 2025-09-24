(module
  (import "wasi_snapshot_preview1" "fd_write" (func $fd_write (param i32 i32 i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "proc_exit" (func $proc_exit (param i32)))
  
  (memory 1)
  (export "memory" (memory 0))
  
  ;; Data segment with "Hello from WASM Container!\n"
  (data (i32.const 8) "Hello from WASM Container!\n")
  
  (func $main (export "_start")
    ;; Create iovec structure at memory offset 0
    (i32.store (i32.const 0) (i32.const 8))   ;; iov_base = 8 (string location)
    (i32.store (i32.const 4) (i32.const 28))  ;; iov_len = 28 (string length)
    
    ;; Call fd_write(stdout=1, iovs=0, iovs_len=1, nwritten=100)
    (call $fd_write
      (i32.const 1)    ;; stdout
      (i32.const 0)    ;; pointer to iovec array
      (i32.const 1)    ;; number of iovecs
      (i32.const 100)  ;; where to store bytes written
    )
    drop
    
    ;; Exit with code 0
    (i32.const 0)
    (call $proc_exit)
  )
)