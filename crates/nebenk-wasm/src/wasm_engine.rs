use nebenk_core::error::{NebenkError, Result};
use nebenk_core::traits::StateEngine;
use nebenk_core::types::{ApplyResult, OperationEnvelope, Snapshot};
use wasmtime::{Engine, Instance, Memory, Module, Store, TypedFunc};

/// Sandboxed WebAssembly guest execution engine powered by Wasmtime.
pub struct WasmEngine {
    _engine: Engine,
    store: Store<()>,
    _instance: Instance,
    memory: Memory,
    alloc_fn: TypedFunc<i32, i32>,
    free_fn: TypedFunc<(i32, i32), ()>,
    apply_fn: TypedFunc<(i32, i32, i64, i64), i32>,
    snapshot_fn: TypedFunc<i32, i32>,
    restore_fn: TypedFunc<(i32, i32), i32>,
    _query_fn: TypedFunc<(i32, i32), i32>,
    current_rev: u64,
}

impl WasmEngine {
    pub fn from_wasm_bytes(wasm_bytes: &[u8]) -> Result<Self> {
        let engine = Engine::default();
        let module = Module::new(&engine, wasm_bytes)
            .map_err(|e| NebenkError::Wasm(format!("Failed to compile WASM module: {e}")))?;

        let mut store = Store::new(&engine, ());
        let instance = Instance::new(&mut store, &module, &[])
            .map_err(|e| NebenkError::Wasm(format!("Failed to instantiate WASM module: {e}")))?;

        let memory = instance
            .get_memory(&mut store, "memory")
            .ok_or_else(|| NebenkError::Wasm("Missing exported 'memory' in WASM module".into()))?;

        let alloc_fn = instance
            .get_typed_func::<i32, i32>(&mut store, "nebenk_alloc")
            .map_err(|e| NebenkError::Wasm(format!("Missing 'nebenk_alloc': {e}")))?;

        let free_fn = instance
            .get_typed_func::<(i32, i32), ()>(&mut store, "nebenk_free")
            .map_err(|e| NebenkError::Wasm(format!("Missing 'nebenk_free': {e}")))?;

        let apply_fn = instance
            .get_typed_func::<(i32, i32, i64, i64), i32>(&mut store, "nebenk_apply")
            .map_err(|e| NebenkError::Wasm(format!("Missing 'nebenk_apply': {e}")))?;

        let snapshot_fn = instance
            .get_typed_func::<i32, i32>(&mut store, "nebenk_snapshot")
            .map_err(|e| NebenkError::Wasm(format!("Missing 'nebenk_snapshot': {e}")))?;

        let restore_fn = instance
            .get_typed_func::<(i32, i32), i32>(&mut store, "nebenk_restore")
            .map_err(|e| NebenkError::Wasm(format!("Missing 'nebenk_restore': {e}")))?;

        let _query_fn = instance
            .get_typed_func::<(i32, i32), i32>(&mut store, "nebenk_query")
            .map_err(|e| NebenkError::Wasm(format!("Missing 'nebenk_query': {e}")))?;

        Ok(Self {
            _engine: engine,
            store,
            _instance: instance,
            memory,
            alloc_fn,
            free_fn,
            apply_fn,
            snapshot_fn,
            restore_fn,
            _query_fn,
            current_rev: 0,
        })
    }

    fn write_guest_buffer(&mut self, data: &[u8]) -> Result<(i32, i32)> {
        let len = data.len() as i32;
        let ptr = self
            .alloc_fn
            .call(&mut self.store, len)
            .map_err(|e| NebenkError::Wasm(format!("Allocation failed: {e}")))?;

        self.memory
            .write(&mut self.store, ptr as usize, data)
            .map_err(|e| NebenkError::Wasm(format!("Failed to write to guest memory: {e}")))?;

        Ok((ptr, len))
    }

    fn free_guest_buffer(&mut self, ptr: i32, len: i32) -> Result<()> {
        self.free_fn
            .call(&mut self.store, (ptr, len))
            .map_err(|e| NebenkError::Wasm(format!("Free failed: {e}")))
    }
}

impl StateEngine for WasmEngine {
    fn init(&mut self, _config: &[u8]) -> Result<()> {
        self.current_rev = 0;
        Ok(())
    }

    fn apply(&mut self, op: &OperationEnvelope) -> Result<ApplyResult> {
        let expected_rev = self.current_rev + 1;
        if op.revision != expected_rev {
            return Err(NebenkError::InvalidRevision {
                expected: expected_rev,
                actual: op.revision,
            });
        }

        let (op_ptr, op_len) = self.write_guest_buffer(&op.payload)?;
        let res = self.apply_fn.call(
            &mut self.store,
            (
                op_ptr,
                op_len,
                op.revision as i64,
                op.timestamp_ms as i64,
            ),
        );
        let _ = self.free_guest_buffer(op_ptr, op_len);

        let code = res.map_err(|e| NebenkError::Wasm(format!("Execution failed: {e}")))?;
        if code != 0 {
            return Err(NebenkError::Wasm(format!("nebenk_apply returned non-zero code: {code}")));
        }

        self.current_rev = op.revision;
        let snap = self.snapshot()?;
        let state_hash = Snapshot::compute_hash(&snap);

        Ok(ApplyResult {
            revision: self.current_rev,
            output: Vec::new(),
            state_hash,
        })
    }

    fn snapshot(&mut self) -> Result<Vec<u8>> {
        // Allocate space for 8 bytes return descriptor (ptr: i32, len: i32)
        let (out_desc_ptr, out_desc_len) = self.write_guest_buffer(&[0u8; 8])?;
        let res = self
            .snapshot_fn
            .call(&mut self.store, out_desc_ptr)
            .map_err(|e| NebenkError::Wasm(format!("Snapshot failed: {e}")))?;

        if res != 0 {
            let _ = self.free_guest_buffer(out_desc_ptr, out_desc_len);
            return Err(NebenkError::Wasm(format!("nebenk_snapshot returned code {res}")));
        }

        let mut desc_bytes = [0u8; 8];
        self.memory
            .read(&self.store, out_desc_ptr as usize, &mut desc_bytes)
            .map_err(|e| NebenkError::Wasm(format!("Failed to read descriptor: {e}")))?;
        let _ = self.free_guest_buffer(out_desc_ptr, out_desc_len);

        let snap_ptr = i32::from_le_bytes(desc_bytes[0..4].try_into().unwrap());
        let snap_len = i32::from_le_bytes(desc_bytes[4..8].try_into().unwrap());

        let mut snap_data = vec![0u8; snap_len as usize];
        self.memory
            .read(&self.store, snap_ptr as usize, &mut snap_data)
            .map_err(|e| NebenkError::Wasm(format!("Failed to read snapshot data: {e}")))?;

        let _ = self.free_guest_buffer(snap_ptr, snap_len);
        Ok(snap_data)
    }

    fn restore(&mut self, revision: u64, snapshot_data: &[u8]) -> Result<()> {
        let (ptr, len) = self.write_guest_buffer(snapshot_data)?;
        let res = self.restore_fn.call(&mut self.store, (ptr, len));
        let _ = self.free_guest_buffer(ptr, len);

        let code = res.map_err(|e| NebenkError::Wasm(format!("Restore call failed: {e}")))?;
        if code != 0 {
            return Err(NebenkError::Wasm(format!("nebenk_restore returned code: {code}")));
        }

        self.current_rev = revision;
        Ok(())
    }

    fn query(&self, _query_payload: &[u8]) -> Result<Vec<u8>> {
        // Query read-only hook
        Ok(Vec::new())
    }

    fn current_revision(&self) -> u64 {
        self.current_rev
    }
}
