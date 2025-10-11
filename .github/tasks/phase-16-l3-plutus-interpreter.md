# L3 Roadmap - Task 16: Plutus Script Interpreter

**Task 16:** Plutus V1/V2/V3 Script Interpreter and Validation

- **Status**: Not Started
- **Files**:
  - Create: `crates/cardano-plutus/Cargo.toml`
  - Create: `crates/cardano-plutus/src/lib.rs`
  - Create: `crates/cardano-plutus/src/interpreter.rs` (~800 lines)
  - Create: `crates/cardano-plutus/src/builtins.rs` (~600 lines)
  - Create: `crates/cardano-plutus/src/budget.rs` (~400 lines)
  - Create: `crates/cardano-plutus/src/context.rs` (~500 lines)
  - Create: `tests/plutus/interpreter_tests.rs`
- **Description**: Implement Plutus Core interpreter for executing smart contracts, including Untyped Plutus Core (UPLC) evaluation, built-in functions, execution budget tracking, and script context building.

## Task Checklist

### Plutus Core AST

- [ ] Define UPLC term types (Variable, Lambda, Apply, Constant, Builtin, Force, Delay, Error)
- [ ] Implement CBOR deserialization for Plutus scripts
- [ ] Create term representation
- [ ] Add de Bruijn indices support
- [ ] Implement term evaluation semantics
- [ ] Add error handling

### UPLC Interpreter

- [ ] Implement CEK machine (Control, Environment, Kontinuation)
- [ ] Add lambda evaluation
- [ ] Implement function application
- [ ] Handle builtin functions
- [ ] Add delay/force for laziness
- [ ] Implement error propagation
- [ ] Add stack overflow protection

### Built-in Functions

- [ ] Implement cryptographic functions (hash, verify signature)
- [ ] Add arithmetic operations (add, subtract, multiply, divide)
- [ ] Implement comparison operations
- [ ] Add bytestring operations
- [ ] Implement list operations
- [ ] Add data manipulation functions
- [ ] Create serialization/deserialization builtins
- [ ] Implement V2/V3 specific builtins

### Execution Budget (ExUnits)

- [ ] Define ExUnits (CPU and Memory)
- [ ] Implement cost model parameters
- [ ] Add budget tracking during evaluation
- [ ] Create budget exhaustion detection
- [ ] Implement cost calculation per operation
- [ ] Add builtin cost lookup
- [ ] Validate against budget limits

### Script Context Building

- [ ] Create ScriptContext for V1
- [ ] Add ScriptPurpose (Spending, Minting, Rewarding, Certifying)
- [ ] Implement TxInfo construction
- [ ] Add ScriptContext for V2 (reference inputs)
- [ ] Create ScriptContext for V3 (governance)
- [ ] Serialize context to Plutus Data
- [ ] Pass context to scripts

### Datum and Redeemer Handling

- [ ] Define Plutus Data type
- [ ] Implement Data CBOR encoding/decoding
- [ ] Add datum storage and lookup
- [ ] Handle inline datums
- [ ] Process redeemers
- [ ] Validate datum hashes

### Script Validation

- [ ] Integrate interpreter with tx validation
- [ ] Execute spending scripts
- [ ] Execute minting policies
- [ ] Execute reward scripts
- [ ] Execute certificate scripts
- [ ] Handle script failures gracefully
- [ ] Implement two-phase validation

### Testing

- [ ] Test basic UPLC evaluation
- [ ] Test built-in functions
- [ ] Test budget tracking
- [ ] Test script context building
- [ ] Test spending validators
- [ ] Test minting policies
- [ ] Validate against Plutus test vectors
- [ ] Test budget exhaustion
- [ ] Test against real mainnet scripts

## Implementation Overview

```rust
// crates/cardano-plutus/src/lib.rs

pub mod interpreter;
pub mod builtins;
pub mod budget;
pub mod context;

pub use interpreter::{Interpreter, EvalResult};
pub use budget::{ExUnits, ExBudget};
pub use context::ScriptContext;

// crates/cardano-plutus/src/interpreter.rs

#[derive(Debug, Clone)]
pub enum Term {
    Variable(usize),           // de Bruijn index
    Lambda(Box<Term>),
    Apply(Box<Term>, Box<Term>),
    Constant(Constant),
    Builtin(Builtin),
    Force(Box<Term>),
    Delay(Box<Term>),
    Error,
}

#[derive(Debug, Clone)]
pub enum Constant {
    Integer(i128),
    ByteString(Vec<u8>),
    String(String),
    Unit,
    Bool(bool),
    List(Vec<Constant>),
    Pair(Box<Constant>, Box<Constant>),
    Data(Data),
}

pub struct Interpreter {
    budget: ExBudget,
    cost_model: CostModel,
}

impl Interpreter {
    pub fn evaluate(
        &mut self,
        script: &[u8],
        context: &ScriptContext,
    ) -> Result<EvalResult> {
        // Deserialize Plutus script
        let term = self.deserialize_script(script)?;

        // Build script arguments (datum, redeemer, context)
        let args = self.build_arguments(context)?;

        // Apply arguments to script
        let applied = self.apply_arguments(term, args)?;

        // Evaluate with CEK machine
        let result = self.eval_cek(applied)?;

        // Check if result is True (success) or False/Error (failure)
        match result {
            Value::Constant(Constant::Bool(true)) => Ok(EvalResult::Success),
            Value::Constant(Constant::Bool(false)) => Ok(EvalResult::Failure),
            _ => Err(EvalError::InvalidResult),
        }
    }

    fn eval_cek(&mut self, term: Term) -> Result<Value> {
        let mut control = term;
        let mut env = Environment::empty();
        let mut stack = Stack::empty();

        loop {
            // Charge CPU for this step
            self.charge_cpu(1)?;

            match control {
                Term::Variable(idx) => {
                    control = env.lookup(idx)?;
                }
                Term::Lambda(body) => {
                    if let Some(frame) = stack.pop() {
                        match frame {
                            Frame::Apply(arg) => {
                                env.push(arg);
                                control = *body;
                            }
                            Frame::Force => {
                                // Force a delayed computation
                                control = *body;
                            }
                        }
                    } else {
                        return Ok(Value::Lambda(env.clone(), body));
                    }
                }
                Term::Apply(fun, arg) => {
                    stack.push(Frame::Apply(Value::from_term(*arg, env.clone())));
                    control = *fun;
                }
                Term::Constant(c) => {
                    if let Some(frame) = stack.pop() {
                        // Continue with next frame
                        return Err(EvalError::CannotApplyConstant);
                    } else {
                        return Ok(Value::Constant(c));
                    }
                }
                Term::Builtin(b) => {
                    let result = self.eval_builtin(b, &mut stack)?;
                    control = Term::Constant(result);
                }
                Term::Force(term) => {
                    stack.push(Frame::Force);
                    control = *term;
                }
                Term::Delay(term) => {
                    if let Some(Frame::Force) = stack.pop() {
                        control = *term;
                    } else {
                        return Ok(Value::Delay(env, term));
                    }
                }
                Term::Error => {
                    return Err(EvalError::ScriptError);
                }
            }
        }
    }
}

// crates/cardano-plutus/src/builtins.rs

#[derive(Debug, Clone, Copy)]
pub enum Builtin {
    // Arithmetic
    AddInteger,
    SubtractInteger,
    MultiplyInteger,
    DivideInteger,
    QuotientInteger,
    RemainderInteger,
    ModInteger,

    // Comparison
    EqualsInteger,
    LessThanInteger,
    LessThanEqualsInteger,

    // Bytestring
    AppendByteString,
    ConsByteString,
    SliceByteString,
    LengthOfByteString,
    IndexByteString,

    // Cryptographic
    Sha2_256,
    Sha3_256,
    Blake2b_256,
    VerifyEd25519Signature,
    VerifyEcdsaSecp256k1Signature,
    VerifySchnorrSecp256k1Signature,

    // Data
    SerialiseData,
    UnConstrData,
    UnMapData,
    UnListData,
    UnIData,
    UnBData,

    // V2 builtins
    SerialiseData,
    VerifyEcdsaSecp256k1Signature,
    VerifySchnorrSecp256k1Signature,
}

impl Interpreter {
    fn eval_builtin(&mut self, builtin: Builtin, stack: &mut Stack) -> Result<Constant> {
        use Builtin::*;

        match builtin {
            AddInteger => {
                let (a, b) = self.pop_two_integers(stack)?;
                self.charge_builtin_cost(AddInteger, &[a, b])?;
                Ok(Constant::Integer(a + b))
            }
            Blake2b_256 => {
                let bytes = self.pop_bytestring(stack)?;
                self.charge_builtin_cost(Blake2b_256, &[bytes.len() as i128])?;
                let hash = blake2b_256(&bytes);
                Ok(Constant::ByteString(hash.to_vec()))
            }
            VerifyEd25519Signature => {
                let (pubkey, message, signature) = self.pop_signature_args(stack)?;
                self.charge_builtin_cost(VerifyEd25519Signature, &[])?;
                let valid = verify_ed25519(&pubkey, &message, &signature);
                Ok(Constant::Bool(valid))
            }
            // ... implement all other builtins
            _ => Err(EvalError::UnimplementedBuiltin(builtin)),
        }
    }
}

// crates/cardano-plutus/src/budget.rs

#[derive(Debug, Clone, Copy)]
pub struct ExUnits {
    pub mem: u64,
    pub cpu: u64,
}

pub struct ExBudget {
    remaining_mem: u64,
    remaining_cpu: u64,
    cost_model: CostModel,
}

impl ExBudget {
    pub fn new(max_mem: u64, max_cpu: u64, cost_model: CostModel) -> Self {
        Self {
            remaining_mem: max_mem,
            remaining_cpu: max_cpu,
            cost_model,
        }
    }

    pub fn charge(&mut self, mem: u64, cpu: u64) -> Result<()> {
        if cpu > self.remaining_cpu {
            return Err(BudgetError::OutOfCpu);
        }
        if mem > self.remaining_mem {
            return Err(BudgetError::OutOfMemory);
        }

        self.remaining_cpu -= cpu;
        self.remaining_mem -= mem;
        Ok(())
    }

    pub fn consumed(&self) -> ExUnits {
        ExUnits {
            cpu: self.cost_model.max_cpu - self.remaining_cpu,
            mem: self.cost_model.max_mem - self.remaining_mem,
        }
    }
}

// crates/cardano-plutus/src/context.rs

#[derive(Debug, Clone)]
pub struct ScriptContext {
    pub tx_info: TxInfo,
    pub purpose: ScriptPurpose,
}

#[derive(Debug, Clone)]
pub enum ScriptPurpose {
    Spending(TxIn),
    Minting(PolicyId),
    Rewarding(StakeCredential),
    Certifying(Certificate),
}

#[derive(Debug, Clone)]
pub struct TxInfo {
    pub inputs: Vec<TxInInfo>,
    pub reference_inputs: Vec<TxInInfo>, // V2+
    pub outputs: Vec<TxOut>,
    pub fee: Coin,
    pub mint: Value,
    pub dcerts: Vec<Certificate>,
    pub withdrawals: HashMap<StakeCredential, Coin>,
    pub valid_range: ValidityInterval,
    pub signatories: Vec<PubKeyHash>,
    pub redeemers: HashMap<ScriptPurpose, Redeemer>,
    pub datums: HashMap<DatumHash, Datum>,
    pub id: TxId,
}

impl ScriptContext {
    pub fn to_plutus_data(&self) -> Data {
        // Serialize ScriptContext to Plutus Data format
        // This is what gets passed to scripts
        Data::Constr(0, vec![
            self.tx_info.to_data(),
            self.purpose.to_data(),
        ])
    }
}
```

## Success Criteria

- [ ] UPLC interpreter evaluates correctly
- [ ] All built-in functions work
- [ ] Budget tracking prevents runaway scripts
- [ ] Script context builds correctly for all versions
- [ ] Spending validators execute successfully
- [ ] Minting policies validate correctly
- [ ] All tests pass (30+ tests)
- [ ] Validation against Plutus test vectors passes
- [ ] Real mainnet scripts execute correctly
- [ ] Documentation complete

## Dependencies

- cardano-base-rust for crypto
- External: pallas-codec for CBOR

## Estimated Effort

- UPLC interpreter: 20-25 hours
- Built-in functions: 25-30 hours
- Budget tracking: 10-12 hours
- Script context: 15-18 hours
- Integration with validation: 12-15 hours
- Testing and validation: 30-40 hours
- **Total: 112-140 hours** (split into multiple phases if needed)

## Notes

This is the most complex component. Consider splitting into:

- Phase 16a: UPLC Interpreter + Basic Builtins (50-60h)
- Phase 16b: Advanced Builtins + Budget (40-50h)
- Phase 16c: Script Context + Integration (30-40h)
