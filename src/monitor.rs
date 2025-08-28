use std::time::Duration;

#[allow(unreachable_code)]
pub async fn monitor_task(port: u16, poll_interval: u16) {
    tokio::spawn(async move {
        loop {
            let client = reqwest::Client::new();
            client
                .post(format!("http://localhost:{port}/request_scan",))
                .send()
                .await?;

            tokio::time::sleep(Duration::from_secs(poll_interval as u64)).await;
        }
        Ok::<_, anyhow::Error>(())
    });
}
