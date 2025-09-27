
# Implementation Plan: Cardano Node Rust Rewrite

**Branch**: `001-cardano-node-rust-rewrite` | **Date**: 2025-09-27 | **Spec**: [spec.md](./spec.md)
**Input**: Feature specification from `/workspaces/universal/.specify/specs/001-cardano-node-rust-rewrite/spec.md`

## Reference Github repo:
- original haskell node: `https://github.com/IntersectMBO/cardano-node/tree/master`

## Execution Flow (/plan command scope)
```
1. Load feature spec from Input path
   → If not found: ERROR "No feature spec at {path}"
2. Fill Technical Context (scan for NEEDS CLARIFICATION)
   → Detect Project Type from file system structure or context (web=frontend+backend, mobile=app+api)
   → Set Structure Decision based on project type
3. Fill the Constitution Check section based on the content of the constitution document.
4. Evaluate Constitution Check section below
   → If violations exist: Document in Complexity Tracking
   → If no justification possible: ERROR "Simplify approach first"
   → Update Progress Tracking: Initial Constitution Check
5. Execute Phase 0 → research.md
   → If NEEDS CLARIFICATION remain: ERROR "Resolve unknowns"
6. Execute Phase 1 → contracts, data-model.md, quickstart.md, agent-specific template file (e.g., `CLAUDE.md` for Claude Code, `.github/copilot-instructions.md` for GitHub Copilot, `GEMINI.md` for Gemini CLI, `QWEN.md` for Qwen Code or `AGENTS.md` for opencode).
7. Re-evaluate Constitution Check section
   → If new violations: Refactor design, return to Phase 1
   → Update Progress Tracking: Post-Design Constitution Check
8. Plan Phase 2 → Describe task generation approach (DO NOT create tasks.md)
9. STOP - Ready for /tasks command
```

**IMPORTANT**: The /plan command STOPS at step 7. Phases 2-4 are executed by other commands:
- Phase 2: /tasks command creates tasks.md
- Phase 3-4: Implementation execution (manual or via tools)

## Summary
Complete rewrite of the Cardano Node from Haskell to Rust, maintaining 100% functional compatibility while leveraging Rust's performance, memory safety, and modern tooling. The implementation follows a modular crate structure mirroring Haskell packages, with test-driven development ensuring consensus correctness and cryptographic compatibility.

## Technical Context
**Language/Version**: Rust 1.75+ with 2021 edition
**Primary Dependencies**: Tokio (async runtime), ed25519-dalek (crypto), minicbor (serialization), LMDB/RocksDB (storage)
**Storage**: LMDB for immutable data, RocksDB for UTXO state, custom block storage format
**Testing**: Cargo test with proptest for property-based testing, cross-validation with Haskell implementation
**Target Platform**: Linux servers (primary), Docker containers, potential Windows/macOS support
**Project Type**: Single blockchain node project with modular crate architecture
**Performance Goals**: Block validation ≤90% Haskell time, memory usage ≤80% Haskell, tx throughput ≥1000 tx/sec
**Constraints**: Zero consensus divergence tolerance, byte-for-byte CBOR compatibility, 99.9% uptime requirement
**Scale/Scope**: Global network node, ~500K LOC estimated, 9 crates, full Cardano protocol implementation

## Constitution Check
*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

✅ **Functional Parity & Correctness**: Implementation maintains 100% compatibility with Haskell version through TDD and cross-validation
✅ **Modular Architecture**: 9-crate structure mirrors Haskell packages with clear separation of concerns
✅ **Memory Safety & Performance**: Rust's ownership system provides memory safety, async I/O for performance
✅ **Cryptographic Security**: Using battle-tested crates (ed25519-dalek, etc.) with property-based testing
✅ **Protocol Compliance**: Extensive testing against Haskell implementation for consensus compatibility
✅ **Security Requirements**: Zero consensus divergence tolerance, secure key handling, audit trails
✅ **Performance Standards**: Clear benchmarks vs Haskell (≤90% validation time, ≤80% memory usage)
✅ **Development Workflow**: TDD methodology with proptest, continuous integration, code quality gates

## Project Structure

### Documentation (this feature)
```
specs/[###-feature]/
├── plan.md              # This file (/plan command output)
├── research.md          # Phase 0 output (/plan command)
├── data-model.md        # Phase 1 output (/plan command)
├── quickstart.md        # Phase 1 output (/plan command)
├── contracts/           # Phase 1 output (/plan command)
└── tasks.md             # Phase 2 output (/tasks command - NOT created by /plan)
```

### Source Code (repository root)
```
cardano-node-rust/
├── crates/
│   ├── cardano-crypto/        # Ed25519, VRF, hash functions
│   │   ├── src/
│   │   │   ├── ed25519/
│   │   │   ├── vrf/
│   │   │   ├── hash/
│   │   │   ├── bls/
│   │   │   └── lib.rs
│   │   └── tests/
│   ├── cardano-consensus/     # Ouroboros protocol
│   │   ├── src/
│   │   │   ├── ouroboros/
│   │   │   ├── chain_selection/
│   │   │   ├── validation/
│   │   │   └── slots/
│   │   └── tests/
│   ├── cardano-ledger/        # Transaction processing
│   │   ├── src/
│   │   │   ├── byron/
│   │   │   ├── shelley/
│   │   │   ├── alonzo/
│   │   │   └── babbage/
│   │   └── tests/
│   ├── cardano-network/       # P2P networking
│   │   ├── src/
│   │   │   ├── p2p/
│   │   │   ├── protocols/
│   │   │   └── handshake/
│   │   └── tests/
│   ├── cardano-storage/       # Database operations
│   │   ├── src/
│   │   │   ├── immutable/
│   │   │   ├── volatile/
│   │   │   └── ledger/
│   │   └── tests/
│   ├── cardano-tracing/       # Logging and metrics
│   ├── cardano-api/           # REST API layer
│   ├── cardano-node/          # Main executable
│   ├── cardano-testnet/       # Testing utilities
│   └── cardano-cli/           # Command line interface
├── tests/                     # Integration tests
│   ├── consensus/
│   ├── crypto/
│   ├── ledger/
│   ├── network/
│   └── serialization/
├── scripts/                   # Build and deployment
├── docs/                      # Documentation
└── config/                    # Configuration files
```

**Structure Decision**: Modular crate architecture mirroring the Haskell Cardano Node structure. Each crate has a single responsibility with clear interfaces, enabling independent development and testing. The structure supports the constitutional requirement for modular design while maintaining compatibility with the original implementation.

## Phase 0: Outline & Research
1. **Extract unknowns from Technical Context** above:
   - For each NEEDS CLARIFICATION → research task
   - For each dependency → best practices task
   - For each integration → patterns task

2. **Generate and dispatch research agents**:
   ```
   For each unknown in Technical Context:
     Task: "Research {unknown} for {feature context}"
   For each technology choice:
     Task: "Find best practices for {tech} in {domain}"
   ```

3. **Consolidate findings** in `research.md` using format:
   - Decision: [what was chosen]
   - Rationale: [why chosen]
   - Alternatives considered: [what else evaluated]

**Output**: research.md with all NEEDS CLARIFICATION resolved

## Phase 1: Design & Contracts
*Prerequisites: research.md complete*

1. **Extract entities from feature spec** → `data-model.md`:
   - Entity name, fields, relationships
   - Validation rules from requirements
   - State transitions if applicable

2. **Generate API contracts** from functional requirements:
   - For each user action → endpoint
   - Use standard REST/GraphQL patterns
   - Output OpenAPI/GraphQL schema to `/contracts/`

3. **Generate contract tests** from contracts:
   - One test file per endpoint
   - Assert request/response schemas
   - Tests must fail (no implementation yet)

4. **Extract test scenarios** from user stories:
   - Each story → integration test scenario
   - Quickstart test = story validation steps

5. **Update agent file incrementally** (O(1) operation):
   - Run `.specify/scripts/bash/update-agent-context.sh copilot`
     **IMPORTANT**: Execute it exactly as specified above. Do not add or remove any arguments.
   - If exists: Add only NEW tech from current plan
   - Preserve manual additions between markers
   - Update recent changes (keep last 3)
   - Keep under 150 lines for token efficiency
   - Output to repository root

**Output**: data-model.md, /contracts/*, failing tests, quickstart.md, agent-specific file

## Phase 2: Task Planning Approach
*This section describes what the /tasks command will do - DO NOT execute during /plan*

**Task Generation Strategy**:
- Load existing tasks.md as base (already contains comprehensive implementation tasks)
- Validate current tasks align with Phase 1 design (data model, contracts, quickstart)
- Ensure TDD methodology: property tests and test vectors before crypto implementation
- Cross-reference with constitution requirements for consensus compatibility
- Verify task dependencies match crate architecture (crypto → consensus → ledger → network)

**Ordering Strategy**:
- Foundation first: Cryptographic operations with Haskell compatibility validation
- Test-driven: Property tests and compatibility tests before implementation
- Dependency order: crypto → consensus → ledger → network → storage → node
- Parallel execution: Mark independent crates/modules as [P] for concurrent development
- Integration last: End-to-end testing and mainnet compatibility validation

**Current State Analysis**:
- Existing tasks.md contains ~70 tasks across 7 phases
- Currently in Phase 3.3 (Cryptographic Implementation)
- T016 Ed25519 complete, T017 VRF in progress
- Task structure aligns well with constitutional requirements and design

**Estimated Additions**: 5-10 new tasks for enhanced testing and validation based on design contracts**IMPORTANT**: This phase is executed by the /tasks command, NOT by /plan

## Phase 3+: Future Implementation
*These phases are beyond the scope of the /plan command*

**Phase 3**: Task execution (/tasks command creates tasks.md)
**Phase 4**: Implementation (execute tasks.md following constitutional principles)
**Phase 5**: Validation (run tests, execute quickstart.md, performance validation)

## Complexity Tracking
*Fill ONLY if Constitution Check has violations that must be justified*

| Violation | Why Needed | Simpler Alternative Rejected Because |
|-----------|------------|-------------------------------------|
| [e.g., 4th project] | [current need] | [why 3 projects insufficient] |
| [e.g., Repository pattern] | [specific problem] | [why direct DB access insufficient] |


## Progress Tracking
*This checklist is updated during execution flow*

**Phase Status**:
- [x] Phase 0: Research complete (/plan command)
- [x] Phase 1: Design complete (/plan command)
- [x] Phase 2: Task planning complete (/plan command - describe approach only)
- [ ] Phase 3: Tasks generated (/tasks command)
- [ ] Phase 4: Implementation complete
- [ ] Phase 5: Validation passed

**Gate Status**:
- [x] Initial Constitution Check: PASS
- [x] Post-Design Constitution Check: PASS
- [x] All NEEDS CLARIFICATION resolved
- [x] Complexity deviations documented

---
*Based on Constitution v2.1.1 - See `/memory/constitution.md`*
