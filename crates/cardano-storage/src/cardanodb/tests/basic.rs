use crate::cardanodb::{CardanoDB, CardanoDBConfig};
use tempfile::tempdir;

#[tokio::test]
async fn cardanodb_collect_stats_on_empty_database() {
    let temp = tempdir().expect("tempdir");
    let config = CardanoDBConfig::new(temp.path().to_path_buf());

    let db = CardanoDB::open(config).await.expect("open empty CardanoDB");

    let stats = db.collect_stats().await.expect("collect stats");

    assert_eq!(stats.total_blocks, 0);
    assert_eq!(stats.immutable_blocks, 0);
    assert_eq!(stats.volatile_blocks, 0);
    assert_eq!(stats.chain_height, 0);
    assert_eq!(stats.tip_slot, 0);
    assert_eq!(stats.utxo_entries, 0);
    assert_eq!(stats.ledger_snapshot_count, 0);
    // Newly created directories may exist but should not contain data yet.
    assert_eq!(stats.database_size_bytes, 0);
}
