#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use chainwatch_rs::application::ports::EventStore;
use chainwatch_rs::infrastructure::persistence::Repository;
use sqlx::postgres::PgPoolOptions;

use common::{cleanup_chain, test_header, test_transfer, unique_chain_id};

#[tokio::test]
#[ignore = "integration test: requires PostgreSQL (DATABASE_URL)"]
async fn postgres_migrations_idempotent_persist_and_checkpoint() {
    let url = common::required_database_url();

    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&url)
        .await
        .expect("failed to connect to PostgreSQL");
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("migrate");

    let repo = Repository::new(pool.clone());
    let chain_id = unique_chain_id();
    let header = test_header(10, 0x10);
    let event = test_transfer(&header, 0, 99);

    let first = repo
        .persist_block(chain_id, &header, std::slice::from_ref(&event))
        .await
        .expect("persist 1");
    assert_eq!(first.inserted_events, 1);
    assert_eq!(first.duplicate_events, 0);

    let second = repo
        .persist_block(chain_id, &header, std::slice::from_ref(&event))
        .await
        .expect("persist 2");
    assert_eq!(second.inserted_events, 0);
    assert_eq!(second.duplicate_events, 1);

    let count = repo.count_events(chain_id).await.expect("count");
    assert_eq!(count, 1);

    let checkpoint = repo
        .get_checkpoint(chain_id)
        .await
        .expect("cp")
        .expect("exists");
    assert_eq!(checkpoint.last_indexed_block, 10);
    assert_eq!(checkpoint.last_indexed_block_hash, header.hash_hex());

    cleanup_chain(&pool, chain_id).await;
}

#[tokio::test]
#[ignore = "integration test: requires PostgreSQL (DATABASE_URL)"]
async fn simulated_crash_replay_does_not_duplicate_or_skip() {
    let url = common::required_database_url();
    let pool = PgPoolOptions::new()
        .connect(&url)
        .await
        .expect("failed to connect to PostgreSQL");
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("migrate");
    let repo = Repository::new(pool.clone());
    let chain_id = unique_chain_id();

    let block_1 = test_header(1, 0x01);
    repo.persist_block(chain_id, &block_1, &[test_transfer(&block_1, 0, 1)])
        .await
        .expect("block 1");

    repo.persist_block(chain_id, &block_1, &[test_transfer(&block_1, 0, 1)])
        .await
        .expect("replay block 1");

    let block_2 = test_header(2, 0x02);
    repo.persist_block(chain_id, &block_2, &[test_transfer(&block_2, 0, 2)])
        .await
        .expect("block 2");

    assert_eq!(repo.count_events(chain_id).await.unwrap(), 2);
    let checkpoint = repo.get_checkpoint(chain_id).await.unwrap().unwrap();
    assert_eq!(checkpoint.last_indexed_block, 2);

    cleanup_chain(&pool, chain_id).await;
}
