# Indexing pipeline

The indexer is a Tokio task that polls the chain tip, fetches a bounded window
of blocks, and persists them in order.

## Loop

```text
loop {
    select {
        shutdown => stop
        poll interval => sync_once()
    }
}
```

`sync_once`:

1. Read the tip (`eth_blockNumber`).
2. Detect a reorg against the checkpoint hash.
3. Compute `[next_block, safe_head]`.
4. Split that range into `BATCH_SIZE` windows.
5. For each window, fetch concurrently, persist sequentially.

Tokio is an event loop, like Node's libuv, plus a thread pool. `async fn`
compiles to a state machine. `.await` yields so other tasks can run. A future
does nothing until it is polled.

## Bounded concurrency and backpressure

Unbounded `tokio::spawn` is the analogue of firing millions of promises with
no `p-limit`. It will exhaust RPC provider rate limits and file descriptors.

This project caps in-flight RPC with a `Semaphore` of size
`MAX_CONCURRENT_REQUESTS`. Header fetches for a batch run inside a
`JoinSet`. `eth_getLogs` for the same range also takes a permit.

When the semaphore is fully acquired, extra work waits. That wait **is**
backpressure. Persistence is even stricter — the next batch is not fetched
until the current batch has been written.

The HTTP client is reused (`reqwest::Client`). Tasks are not spawned without a
JoinSet or the indexer loop itself.

## Ordered persistence after concurrent fetch

Network responses complete out of order. Checkpoints cannot. If we wrote block
12's checkpoint before block 11's events, a crash would skip 11 forever.

The processor returns `Vec<BlockBatch>` sorted by block number. The indexer
then calls `persist_block` one block at a time. Empty blocks still get an
`indexed_blocks` row and a checkpoint advance so we do not rescan them.

If a parent hash does not connect to the previous header, the batch is aborted
and the next tick runs reorg detection.

## Checkpoint recovery

On startup the process reads `indexer_checkpoints` for `CHAIN_ID`. If a row
exists, indexing resumes at `last_indexed_block + 1`. `START_BLOCK` is used
only when there is no checkpoint.

## Cancellation

`CancellationToken` is shared by the indexer loop and Axum's graceful
shutdown. SIGINT/SIGTERM cancel the token. In-flight `persist_block`
transactions finish or roll back in Postgres; the loop stops starting new
batches when the token is cancelled.

## Ports

`ChainReader` and `EventStore` exist because tests (and the HTTP API) need a
seam. They are small traits, not a generic repository framework.
