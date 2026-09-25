# Reorganization handling

A reorg is not a database bug. Miners/validators can replace a recent block
with another block at the same height. The old receipts, including Transfer
logs, are no longer canonical.

## Detection

On each indexing tick:

1. Load the checkpoint hash.
2. Ask RPC for that block number.
3. If hashes differ (or the block is gone), walk backward through
   `indexed_blocks` comparing hashes to RPC until they match.
4. That match is the common ancestor.
5. Delete rows with `block_number > ancestor` and reset the checkpoint.

`CONFIRMATION_DEPTH` makes this rare. `MAX_REORG_DEPTH` is a safety limit so a
pathological mismatch cannot scan the entire history in a tight loop.

## Genesis mismatch

If the walk reaches genesis and even block 0 disagrees (typical after a fresh
Anvil process on the same `CHAIN_ID`), the indexer **resets** that chain's
rows in one transaction and starts from `START_BLOCK`. That is a chain
replacement, not a short reorg.

## Parent-hash abort

While persisting a fetched batch, if `next.parent_hash` is not the previous
header's hash, the batch is abandoned. The next poll runs full reorg
detection rather than writing a disconnected chain.

## Tests

`find_common_ancestor` is a pure function covered by unit tests.
`reorg_rollback_uses_database_transaction` persists three blocks, swaps the
canonical hash at the tip via a mock `ChainReader`, walks to the ancestor,
and asserts the SQL rollback deleted exactly the orphaned block.
