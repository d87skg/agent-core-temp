#[tokio::test]
async fn test_redis_scheduler_retry_and_dead_letter() {
    let scheduler = RedisScheduler::new(
        "redis://127.0.0.1:6379",
        "test-stream",
        "test-group",
        "test-worker",
        3,
    )
    .await
    .unwrap();

    let task = Task::new("test", b"payload".to_vec(), 5, None);
    let task_id = scheduler.submit(task.clone()).await.unwrap();

    // 第一次消费
    let popped = scheduler.pop().await.unwrap().unwrap();
    assert_eq!(popped.id, task_id);

    // 第一次失败
    scheduler.nack(&task_id, "error 1").await.unwrap();

    // 第二次消费（应收到重试任务）
    let popped = scheduler.pop().await.unwrap().unwrap();
    assert_eq!(popped.retry_count, 1);

    // 再次失败
    scheduler.nack(&task_id, "error 2").await.unwrap();

    // 第三次消费
    let popped = scheduler.pop().await.unwrap().unwrap();
    assert_eq!(popped.retry_count, 2);

    // 第三次失败（达到最大重试）
    scheduler.nack(&task_id, "error 3").await.unwrap();

    // 第四次 pop 应无任务
    let popped = scheduler.pop().await.unwrap();
    assert!(popped.is_none());

    // 死信队列应有 1 条
    assert_eq!(scheduler.dead_letter_count().await.unwrap(), 1);
}