# Data Model: Cardano Node Rust Rewrite

## Core Blockchain Entities

### Block
**Fields**:
- `header: BlockHeader` - Block metadata and consensus information
- `body: BlockBody` - Transaction data and auxiliary information
- `hash: Blake2b256` - Unique block identifier
- `size: u64` - Block size in bytes

**Relationships**:
- Parent: `previous_hash` in header points to parent block
- Children: Referenced by subsequent blocks
- Transactions: Contains ordered list of transactions

**Validation Rules**:
- Header hash must match computed hash of header
- Previous hash must reference valid parent block
- Block size must not exceed protocol parameter
- All transactions in body must be valid

**State Transitions**:
- New → Validating → Valid/Invalid
- Valid blocks added to chain candidate
- Invalid blocks rejected and peers penalized

### Transaction
**Fields**:
- `inputs: Vec<TransactionInput>` - UTXO references being spent
- `outputs: Vec<TransactionOutput>` - New UTXOs being created
- `fee: Coin` - Transaction fee paid to stake pool
- `ttl: Option<Slot>` - Optional time-to-live
- `certificates: Vec<Certificate>` - Staking and governance certificates
- `withdrawals: Map<RewardAddress, Coin>` - Reward withdrawals
- `auxiliary_data: Option<AuxiliaryData>` - Metadata and scripts
- `validity_interval: ValidityInterval` - Valid slot range
- `witness_set: TransactionWitnessSet` - Cryptographic proofs

**Relationships**:
- Inputs: References to existing UTXOs
- Outputs: Creates new UTXOs
- Block: Contained within exactly one block

**Validation Rules**:
- Sum of inputs ≥ sum of outputs + fee
- All input UTXOs must exist and be unspent
- All signatures must be valid
- Script execution must succeed (for script-locked UTXOs)
- Validity interval must contain current slot

### UTXO (Unspent Transaction Output)
**Fields**:
- `transaction_id: TransactionId` - Creating transaction
- `output_index: u32` - Index within transaction
- `address: Address` - Payment/script address
- `amount: Value` - Coin and native assets
- `datum: Option<Datum>` - Smart contract state
- `script_ref: Option<ScriptRef>` - Referenced script

**Relationships**:
- Transaction: Created by transaction output
- Input: Can be spent as transaction input

**Validation Rules**:
- Amount must be positive
- Address must be valid for current era
- Datum hash must match if present

### Address
**Variants**:
- `Byron(ByronAddress)` - Legacy Byron era addresses
- `Shelley(ShelleyAddress)` - Modern payment addresses
- `Pointer(PointerAddress)` - Pointer to staking credential
- `Enterprise(EnterpriseAddress)` - No staking component
- `Reward(RewardAddress)` - Staking reward addresses

**Fields** (Shelley variant):
- `network: NetworkId` - Mainnet/testnet identifier
- `payment_credential: Credential` - Payment authorization
- `stake_credential: Option<Credential>` - Optional staking authorization

## Consensus Entities

### BlockHeader
**Fields**:
- `previous_hash: Option<Blake2b256>` - Hash of previous block (None for genesis)
- `issuer_vkey: VKey` - Block producer verification key
- `vrf_vkey: VrfVKey` - VRF verification key
- `nonce_vrf: VrfProof` - VRF proof for epoch nonce
- `leader_vrf: VrfProof` - VRF proof for leader election
- `block_size: u32` - Size of block body
- `block_body_hash: Blake2b256` - Hash of block body
- `operational_cert: OperationalCert` - Hot key authorization
- `protocol_version: ProtocolVersion` - Protocol version used

**Validation Rules**:
- VRF proofs must verify correctly
- Operational certificate must be valid
- Block producer must be scheduled leader for slot
- Protocol version must be supported

### ChainState
**Fields**:
- `tip: BlockHash` - Current chain tip
- `slot: Slot` - Current slot number
- `epoch: Epoch` - Current epoch
- `utxo_set: UtxoSet` - All unspent transaction outputs
- `stake_distribution: StakeDistribution` - Current stake pools and delegation
- `protocol_params: ProtocolParameters` - Current protocol parameters

**State Transitions**:
- Block application: Updates UTXO set, advances slot
- Epoch transition: Updates stake distribution, potentially protocol params
- Rollback: Reverts to previous valid state

### StakePool
**Fields**:
- `pool_id: PoolId` - Unique pool identifier
- `vrf_key: VrfVKey` - VRF verification key for leader election
- `pledge: Coin` - Pool operator pledge amount
- `cost: Coin` - Fixed cost per epoch
- `margin: Rational` - Variable fee percentage
- `reward_account: RewardAddress` - Pool operator reward address
- `owners: Set<KeyHash>` - Pool owner key hashes
- `relays: Vec<Relay>` - Network relay information
- `metadata: Option<PoolMetadata>` - Pool description and branding

## Cryptographic Entities

### Ed25519Key
**Fields**:
- `private_key: Option<[u8; 32]>` - Private key (secret)
- `public_key: [u8; 32]` - Public key
- `signature: Option<[u8; 64]>` - Digital signature

**Operations**:
- `generate() -> Self` - Generate new keypair
- `sign(message: &[u8]) -> Signature` - Create signature
- `verify(message: &[u8], sig: &Signature) -> bool` - Verify signature

### VrfKey
**Fields**:
- `private_key: Option<[u8; 32]>` - VRF private key (secret)
- `public_key: [u8; 32]` - VRF public key
- `proof: Option<[u8; 81]>` - VRF proof

**Operations**:
- `prove(input: &[u8]) -> (VrfOutput, VrfProof)` - Generate VRF proof
- `verify(input: &[u8], output: &VrfOutput, proof: &VrfProof) -> bool` - Verify proof

### Hash
**Variants**:
- `Blake2b224([u8; 28])` - 224-bit Blake2b hash
- `Blake2b256([u8; 32])` - 256-bit Blake2b hash
- `Sha256([u8; 32])` - 256-bit SHA-2 hash

**Operations**:
- `digest(data: &[u8]) -> Self` - Compute hash
- `as_bytes(&self) -> &[u8]` - Get raw bytes

## Network Protocol Entities

### PeerState
**Fields**:
- `peer_id: PeerId` - Unique peer identifier
- `address: SocketAddr` - Network address
- `protocol_version: ProtocolVersion` - Negotiated protocol version
- `connection_state: ConnectionState` - Current connection status
- `last_seen: SystemTime` - Last successful communication
- `reputation: i32` - Peer reputation score

**State Transitions**:
- Disconnected → Connecting → Connected → Authenticated
- Connected → Disconnected (on error or timeout)
- Reputation adjustments based on behavior

### Message
**Variants**:
- `Handshake(HandshakeMessage)` - Initial connection setup
- `ChainSync(ChainSyncMessage)` - Block synchronization protocol
- `BlockFetch(BlockFetchMessage)` - Block retrieval protocol
- `TxSubmission(TxSubmissionMessage)` - Transaction submission
- `KeepAlive` - Connection maintenance

**Serialization**: CBOR encoding for network transmission

## Storage Entities

### Database Schema
**Tables/Collections**:
- `blocks` - Block storage with hash indexing
- `transactions` - Transaction storage with ID indexing
- `utxos` - UTXO set with address and reference indexing
- `stake_pools` - Pool registration and updates
- `chain_state` - Current chain tip and epoch info

**Indexing Strategy**:
- Primary: Block hash, transaction ID, UTXO reference
- Secondary: Block height, slot number, address
- Composite: Address + asset ID for multi-asset queries

### CacheLayer
**Fields**:
- `recent_blocks: LruCache<BlockHash, Block>` - Recently accessed blocks
- `utxo_cache: LruCache<UtxoRef, Utxo>` - Hot UTXO set
- `pool_cache: HashMap<PoolId, StakePool>` - Active stake pools

**Cache Policies**:
- Block cache: LRU eviction, 1000 block limit
- UTXO cache: Write-through with periodic flush
- Pool cache: Full epoch retention

## Era-Specific Adaptations

### Byron Era
- Legacy address format
- No native assets or smart contracts
- Different transaction structure

### Shelley Era
- Modern address format
- Staking and delegation
- Multi-signature support

### Allegra Era
- Native assets introduction
- Asset minting policies

### Mary Era
- Full multi-asset support
- Asset metadata

### Alonzo Era
- Plutus smart contracts
- Script data and redeemers
- Collateral inputs

### Babbage Era
- Reference scripts
- Inline datums
- Reference inputs

### Conway Era
- Governance actions
- Voting and treasury

## Validation State Machine

### BlockValidation
**States**:
- `Initial` - Block received, basic checks pending
- `HeaderValidated` - Header cryptographically valid
- `BodyValidated` - All transactions valid
- `ConsensusValidated` - Consensus rules verified
- `Applied` - Block added to chain
- `Rejected` - Validation failed

**Transitions**:
- Each state transition requires specific validation checks
- Failure at any stage moves to Rejected state
- Only Applied blocks affect chain state

### TransactionValidation
**Phases**:
1. **Structural**: CBOR parsing and well-formedness
2. **Semantic**: Balance, UTXO existence, script execution
3. **Consensus**: Slot validity, fee sufficiency
4. **Application**: UTXO set updates

**Error Handling**:
- Validation errors include context for debugging
- Failed transactions don't affect other validations
- Detailed error reporting for transaction submitters
