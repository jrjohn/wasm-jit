(module
 (type $0 (func (param f64 f64 f64)))
 (type $1 (func (param f64)))
 (type $2 (func (param f64 f64 f64 f64 f64)))
 (type $3 (func (param f64) (result f64)))
 (type $4 (func (param f64 f64 f64) (result f64)))
 (type $5 (func (param f64 f64 f64 f64)))
 (import "env" "hue" (func $assembly/buddha/hue (param f64)))
 (import "env" "disc" (func $assembly/buddha/disc (param f64 f64 f64)))
 (import "env" "arc" (func $assembly/buddha/arc (param f64 f64 f64 f64 f64)))
 (import "env" "sin" (func $assembly/buddha/sin (param f64) (result f64)))
 (import "env" "ring" (func $assembly/buddha/ring (param f64 f64 f64)))
 (import "env" "line" (func $assembly/buddha/line (param f64 f64 f64 f64)))
 (memory $0 0)
 (export "run" (func $assembly/buddha/run))
 (export "memory" (memory $0))
 (func $assembly/buddha/run (param $0 f64) (param $1 f64) (param $2 f64) (result f64)
  (local $3 i32)
  (local $4 f64)
  (local $5 f64)
  (local $6 f64)
  (local $7 f64)
  f64.const 0.13
  call $assembly/buddha/hue
  local.get $1
  f64.const 0.5
  f64.mul
  local.tee $6
  local.get $2
  f64.const 0.56
  f64.mul
  local.tee $1
  local.get $2
  f64.const 0.27
  f64.mul
  local.tee $4
  f64.const 0.18
  f64.mul
  f64.sub
  local.get $4
  f64.const 1.55
  f64.mul
  local.get $0
  call $assembly/buddha/sin
  f64.const 6
  f64.mul
  f64.add
  call $assembly/buddha/ring
  f64.const 0.09
  call $assembly/buddha/hue
  local.get $6
  local.get $1
  local.get $4
  f64.const 1.04
  f64.mul
  f64.sub
  local.get $4
  f64.const 0.2
  f64.mul
  call $assembly/buddha/disc
  f64.const 0.1
  call $assembly/buddha/hue
  local.get $6
  local.get $1
  local.get $4
  call $assembly/buddha/disc
  f64.const 0.1
  call $assembly/buddha/hue
  loop $for-loop|0
   local.get $3
   i32.const 2
   i32.lt_s
   if
    local.get $6
    f64.const 1
    f64.const -1
    local.get $3
    select
    local.get $4
    f64.mul
    f64.const 1.06
    f64.mul
    f64.add
    local.tee $2
    local.get $1
    local.get $4
    f64.const 0.15
    f64.mul
    f64.sub
    local.get $2
    local.get $1
    local.get $4
    f64.const 0.5
    f64.mul
    f64.add
    call $assembly/buddha/line
    local.get $3
    i32.const 1
    i32.add
    local.set $3
    br $for-loop|0
   end
  end
  f64.const 0.99
  call $assembly/buddha/hue
  local.get $6
  local.get $1
  local.get $4
  f64.const 0.28
  f64.mul
  f64.sub
  local.get $4
  f64.const 0.045
  f64.mul
  call $assembly/buddha/disc
  f64.const 0.02
  call $assembly/buddha/hue
  local.get $6
  local.get $4
  f64.const 0.36
  f64.mul
  local.tee $5
  f64.sub
  local.get $1
  local.get $4
  f64.const 0.02
  f64.mul
  f64.sub
  local.tee $7
  local.get $4
  f64.const 0.17
  f64.mul
  local.tee $2
  f64.const 3.35
  f64.const 6.05
  call $assembly/buddha/arc
  local.get $6
  local.get $5
  f64.add
  local.get $7
  local.get $2
  f64.const 3.35
  f64.const 6.05
  call $assembly/buddha/arc
  local.get $6
  local.get $1
  local.get $4
  f64.const 0.22
  f64.mul
  f64.add
  local.get $4
  f64.const 0.46
  f64.mul
  local.get $0
  local.get $0
  f64.add
  call $assembly/buddha/sin
  f64.const 3
  f64.mul
  f64.add
  f64.const 0.45
  f64.const 2.69
  call $assembly/buddha/arc
  f64.const 0
 )
)
